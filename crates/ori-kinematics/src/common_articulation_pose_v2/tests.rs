//! Contract and fixture tests for the general-N V2 pose issuer.
use std::collections::{BTreeMap, BTreeSet, HashSet};

use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_topology::{FaceExtractionInput, analyze_faces};

use super::*;
use crate::{
    CanonicalCycleScheduleV1, CanonicalEdgeBlockLimitsV1, CanonicalHingeAngles,
    CycleScheduleEntryInputV1, CycleScheduleLimitsV1, CycleSchedulePrepareErrorV1,
    CycleScheduleRestrictionErrorV1, CycleScheduleRestrictionStopV1, HingeAngle,
    MaterialHingeGraphAudit, RationalCoefficientV1, TreeKinematicsLimits,
};

#[path = "schedule_closure_tests.rs"]
mod schedule_closure_tests;

#[path = "block_closure_set_tests.rs"]
mod block_closure_set_tests;

#[path = "whole_parent_closure_tests.rs"]
mod whole_parent_closure_tests;

struct MiuraFixtureV2 {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    pose: ClosedMaterialHingeGraphPose,
    decomposition: CanonicalMaterialEdgeBlockDecompositionV2,
}

impl MiuraFixtureV2 {
    fn input<'a>(
        &'a self,
        profile: &'a CommonArticulationResourceProfileV2,
    ) -> CommonArticulationPoseInputV2<'a> {
        CommonArticulationPoseInputV2 {
            geometry: &self.geometry,
            pose: &self.pose,
            decomposition: &self.decomposition,
            paper_thickness_mm: 0.1,
            profile,
        }
    }

    fn new_pose_instance(&self) -> ClosedMaterialHingeGraphPose {
        let angles = zero_angles_v2(&self.geometry);
        self.geometry
            .solve_closed(&self.audit, self.geometry.face_ids()[0], &angles, 0.0)
            .expect("same-geometry fresh pose")
    }

    fn decomposition_with_profile(
        &self,
        profile: &CommonArticulationResourceProfileV2,
    ) -> CanonicalMaterialEdgeBlockDecompositionV2 {
        self.geometry
            .decompose_canonical_edge_blocks_with_profile_v2(&self.audit, profile)
            .expect("same geometry with a profile-bound decomposition")
    }
}

#[test]
fn n33_exact_profile_issues_and_revalidates_a_non_authorizing_pose() {
    let fixture = miura_fixture_v2(33);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33).expect("N33 profile");
    let authority = prove_common_articulation_pose_authority_v2(fixture.input(&profile))
        .expect("N33 V2 pose authority");

    assert_eq!(authority.configured_max_blocks_v2(), 33);
    assert_eq!(authority.actual_block_count_v2(), 33);
    assert_eq!(authority.logical_work_v2(), 18_768);
    assert_eq!(authority.retained_bytes_upper_bound_v2(), 58_048);
    assert_eq!(
        fixture.decomposition.model_id_v2(),
        "common_articulation_edge_block_decomposition_v2"
    );
    assert_eq!(fixture.decomposition.logical_work_v2(), 30_720);
    assert_eq!(
        fixture.decomposition.storage_bytes_upper_bound_v2(),
        1_504_256
    );
    assert_eq!(
        authority.profile_binding_fingerprint_v2(),
        profile.binding_fingerprint_v2()
    );
    assert!(!authority.authorizes_continuous_motion());
    assert!(!authority.authorizes_collision_clearance());
    assert!(!authority.authorizes_project_mutation());
    assert!(!authority.authorizes_apply());
    assert!(!authority.authorizes_viewer());
    assert!(!authority.authorizes_layer_transport());
    let first_block = authority.block_v2(0).expect("first V2 restriction");
    assert!(first_block.is_for_geometry_v2(fixture.decomposition.blocks()[0].geometry()));
    assert!(!first_block.is_for_geometry_v2(&fixture.geometry));
    authority
        .revalidate_v2(fixture.input(&profile))
        .expect("same N33 V2 pose input");
}

#[test]
fn n34_profile_decomposition_and_pose_revalidate_with_independent_bounds() {
    let fixture = miura_fixture_v2(34);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(34).expect("N34 profile");
    let resources = profile.actual_v2();
    // Independent N=34 evaluations of the published canonical-Miura
    // formulae: F=8N+1, H=12N, decomposition work/storage, and
    // pose work/storage.  Do not derive these expected values through
    // another production getter.
    assert_eq!(resources.block_count_v2(), 34);
    assert_eq!(resources.face_count_v2(), 273);
    assert_eq!(resources.hinge_count_v2(), 408);
    assert_eq!(resources.decomposition_logical_work_v2(), 31_648);
    assert_eq!(resources.decomposition_storage_bytes_v2(), 1_549_312);
    assert_eq!(resources.pose_logical_work_v2(), 19_472);
    assert_eq!(resources.pose_retained_bytes_v2(), 59_792);
    assert_eq!(fixture.geometry.face_ids().len(), 273);
    assert_eq!(fixture.geometry.hinges().len(), 408);
    assert_eq!(fixture.decomposition.actual_block_count_v2(), 34);
    assert_eq!(fixture.decomposition.face_count_v2(), 273);
    assert_eq!(fixture.decomposition.hinge_count_v2(), 408);
    assert_eq!(fixture.decomposition.logical_work_v2(), 31_648);
    assert_eq!(
        fixture.decomposition.storage_bytes_upper_bound_v2(),
        1_549_312
    );

    let authority = prove_common_articulation_pose_authority_v2(fixture.input(&profile))
        .expect("N34 V2 pose authority");
    assert_eq!(authority.configured_max_blocks_v2(), 34);
    assert_eq!(authority.actual_block_count_v2(), 34);
    assert_eq!(authority.logical_work_v2(), 19_472);
    assert_eq!(authority.retained_bytes_upper_bound_v2(), 59_792);
    authority
        .revalidate_v2(fixture.input(&profile))
        .expect("same N34 V2 pose input");
}

#[test]
fn n40_cap_n34_actual_profile_isolated_from_exact_n34_authorities() {
    let fixture = miura_fixture_v2(34);
    let exact = CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(34)
        .expect("exact N34 profile");
    let cross_cap = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(40, 34)
        .expect("N40 cap, N34 actual profile");
    let actual = cross_cap.actual_v2();
    let maximum = cross_cap.maximum_v2();

    // Independently fixed cardinalities for this cross-cap case.  The
    // maximum is N=40 while every issued value is still constrained to the
    // N=34 source graph.
    assert_eq!(cross_cap.configured_max_blocks_v2(), 40);
    assert_eq!(cross_cap.actual_block_count_v2(), 34);
    assert_eq!(actual.block_count_v2(), 34);
    assert_eq!(actual.face_count_v2(), 273);
    assert_eq!(actual.hinge_count_v2(), 408);
    assert_eq!(maximum.block_count_v2(), 40);
    assert_eq!(maximum.face_count_v2(), 321);
    assert_eq!(maximum.hinge_count_v2(), 480);
    assert_eq!(fixture.geometry.face_ids().len(), 273);
    assert_eq!(fixture.geometry.hinges().len(), 408);
    assert!(actual.unordered_face_pair_count_v2() <= maximum.unordered_face_pair_count_v2());
    assert!(
        actual.raw_cross_block_pair_candidates_v2() <= maximum.raw_cross_block_pair_candidates_v2()
    );
    assert!(actual.canonical_cross_block_pairs_v2() <= maximum.canonical_cross_block_pairs_v2());
    assert!(
        actual.raw_sort_comparisons_per_item_v2() <= maximum.raw_sort_comparisons_per_item_v2()
    );
    assert!(
        actual.canonical_sort_comparisons_per_item_v2()
            <= maximum.canonical_sort_comparisons_per_item_v2()
    );
    assert!(actual.pose_logical_work_v2() <= maximum.pose_logical_work_v2());
    assert!(actual.pose_retained_bytes_v2() <= maximum.pose_retained_bytes_v2());
    assert!(actual.decomposition_logical_work_v2() <= maximum.decomposition_logical_work_v2());
    assert!(actual.decomposition_storage_bytes_v2() <= maximum.decomposition_storage_bytes_v2());
    assert!(actual.clearance_logical_work_v2() <= maximum.clearance_logical_work_v2());
    assert!(actual.clearance_storage_bytes_v2() <= maximum.clearance_storage_bytes_v2());

    let exact_authority = prove_common_articulation_pose_authority_v2(fixture.input(&exact))
        .expect("exact N34 authority");
    let cross_decomposition = fixture.decomposition_with_profile(&cross_cap);
    let cross_input = CommonArticulationPoseInputV2 {
        geometry: &fixture.geometry,
        pose: &fixture.pose,
        decomposition: &cross_decomposition,
        paper_thickness_mm: 0.1,
        profile: &cross_cap,
    };
    let cross_authority =
        prove_common_articulation_pose_authority_v2(cross_input).expect("N40-cap N34 authority");
    assert_eq!(cross_decomposition.limits().max_blocks, 40);
    assert_eq!(cross_decomposition.actual_block_count_v2(), 34);
    assert_eq!(cross_decomposition.face_count_v2(), 273);
    assert_eq!(cross_decomposition.hinge_count_v2(), 408);
    assert!(!cross_decomposition.is_for_profile_v2(&exact));
    assert!(!fixture.decomposition.is_for_profile_v2(&cross_cap));
    cross_authority
        .revalidate_v2(CommonArticulationPoseInputV2 {
            geometry: &fixture.geometry,
            pose: &fixture.pose,
            decomposition: &cross_decomposition,
            paper_thickness_mm: 0.1,
            profile: &cross_cap,
        })
        .expect("same cross-cap input");

    assert_eq!(
        prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
            geometry: &fixture.geometry,
            pose: &fixture.pose,
            decomposition: &cross_decomposition,
            paper_thickness_mm: 0.1,
            profile: &exact,
        })
        .expect_err("cross-cap decomposition under exact profile"),
        CommonArticulationPoseErrorV2::ResourceLimit,
    );
    assert_eq!(
        prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
            geometry: &fixture.geometry,
            pose: &fixture.pose,
            decomposition: &fixture.decomposition,
            paper_thickness_mm: 0.1,
            profile: &cross_cap,
        })
        .expect_err("exact decomposition under cross-cap profile"),
        CommonArticulationPoseErrorV2::ResourceLimit,
    );
    assert_eq!(
        exact_authority
            .revalidate_v2(CommonArticulationPoseInputV2 {
                geometry: &fixture.geometry,
                pose: &fixture.pose,
                decomposition: &cross_decomposition,
                paper_thickness_mm: 0.1,
                profile: &cross_cap,
            })
            .expect_err("cross-cap input must not revalidate exact authority"),
        CommonArticulationPoseErrorV2::IssuerMismatch,
    );
    assert_eq!(
        cross_authority
            .revalidate_v2(fixture.input(&exact))
            .expect_err("exact input must not revalidate cross-cap authority"),
        CommonArticulationPoseErrorV2::IssuerMismatch,
    );
}

#[test]
fn n34_v1_cycle_schedule_is_explicitly_bounded_and_edge_complete() {
    const N34_HINGES: usize = 408;
    const N34_SCHEDULE_WORK: usize = 408;

    let fixture = miura_fixture_v2(34);
    let fixed_face = fixture.geometry.face_ids()[0];
    let entries = zero_cycle_schedule_entries_v2(&fixture.geometry);
    assert_eq!(entries.len(), N34_HINGES);
    let exact_limits = CycleScheduleLimitsV1 {
        max_hinges: N34_HINGES,
        max_degree: 0,
        max_coefficient_bits: 1,
        max_work: N34_SCHEDULE_WORK,
    };
    let schedule = CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixed_face,
        [0.0, 1.0],
        entries.clone(),
        exact_limits,
    )
    .expect("N34 schedule within explicit V1 caller limits");
    assert!(schedule.matches_binding(&fixture.geometry, &fixture.audit, fixed_face));

    let geometry_edges = fixture
        .geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<HashSet<_>>();
    let audit_edges = fixture
        .audit
        .spanning_hinges()
        .iter()
        .chain(fixture.audit.closure_hinges())
        .copied()
        .collect::<HashSet<_>>();
    let pose_edges = fixture
        .pose
        .hinge_angles()
        .as_slice()
        .iter()
        .map(|angle| angle.edge())
        .collect::<HashSet<_>>();
    assert_eq!(geometry_edges.len(), N34_HINGES);
    assert_eq!(audit_edges, geometry_edges);
    assert_eq!(pose_edges, geometry_edges);

    for parameter in [0.0, 1.0] {
        let evaluated = schedule
            .try_evaluate_v1(parameter)
            .expect("N34 schedule endpoint evaluation");
        let evaluated_edges = evaluated
            .as_slice()
            .iter()
            .map(|angle| angle.edge())
            .collect::<HashSet<_>>();
        assert_eq!(evaluated.as_slice().len(), N34_HINGES);
        assert_eq!(evaluated_edges, geometry_edges);
        assert!(
            evaluated
                .as_slice()
                .iter()
                .all(|angle| angle.angle_degrees() == 0.0)
        );
    }

    assert_eq!(
        CanonicalCycleScheduleV1::prepare(
            &fixture.geometry,
            &fixture.audit,
            fixed_face,
            [0.0, 1.0],
            entries.clone(),
            CycleScheduleLimitsV1 {
                max_hinges: N34_HINGES - 1,
                ..exact_limits
            },
        )
        .expect_err("one-short max_hinges also bounds the entry carrier"),
        CycleSchedulePrepareErrorV1::InvalidInput,
    );
    assert_eq!(
        CanonicalCycleScheduleV1::prepare(
            &fixture.geometry,
            &fixture.audit,
            fixed_face,
            [0.0, 1.0],
            entries,
            CycleScheduleLimitsV1 {
                max_work: N34_SCHEDULE_WORK - 1,
                ..exact_limits
            },
        )
        .expect_err("one-short schedule work"),
        CycleSchedulePrepareErrorV1::ResourceLimit,
    );

    let block = &fixture.decomposition.blocks()[0];
    let block_fixed_face = block.geometry().face_ids()[0];
    let restricted = schedule
        .restrict_to_edge_block_with_fixed_face_with_checkpoint_v1(
            &fixture.geometry,
            &fixture.audit,
            block.geometry(),
            block.audit(),
            block_fixed_face,
            || Ok(()),
        )
        .expect("N34 schedule block restriction");
    let restricted_edges = restricted
        .try_evaluate_v1(0.0)
        .expect("restricted schedule endpoint")
        .as_slice()
        .iter()
        .map(|angle| angle.edge())
        .collect::<HashSet<_>>();
    let block_edges = block
        .geometry()
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<HashSet<_>>();
    assert_eq!(restricted_edges, block_edges);
    assert_eq!(
        schedule
            .restrict_to_edge_block_with_fixed_face_with_checkpoint_v1(
                &fixture.geometry,
                &fixture.audit,
                block.geometry(),
                block.audit(),
                block_fixed_face,
                || Err(CycleScheduleRestrictionStopV1::Cancelled),
            )
            .expect_err("restriction start cancellation"),
        CycleScheduleRestrictionErrorV1::Cancelled,
    );
    assert_eq!(
        schedule
            .restrict_to_edge_block_with_fixed_face_with_checkpoint_v1(
                &fixture.geometry,
                &fixture.audit,
                block.geometry(),
                block.audit(),
                block_fixed_face,
                || Err(CycleScheduleRestrictionStopV1::DeadlineExceeded),
            )
            .expect_err("restriction start deadline"),
        CycleScheduleRestrictionErrorV1::DeadlineExceeded,
    );
}

#[test]
fn n33_one_short_and_cross_cap_profiles_are_rejected_or_bound() {
    let fixture = miura_fixture_v2(33);
    let exact =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33).expect("N33 profile");
    let one_short = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(33, 32)
        .expect("one-short actual N profile");
    assert_eq!(
        fixture
            .geometry
            .decompose_canonical_edge_blocks_with_profile_v2(&fixture.audit, &one_short)
            .expect_err("one-short decomposition profile"),
        crate::CommonArticulationDecompositionErrorV2::ResourceLimit,
    );
    assert_eq!(
        prove_common_articulation_pose_authority_v2(fixture.input(&one_short))
            .expect_err("one-short profile"),
        CommonArticulationPoseErrorV2::ResourceLimit,
    );

    let authority = prove_common_articulation_pose_authority_v2(fixture.input(&exact))
        .expect("exact N33 authority");
    let cross_cap = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(34, 33)
        .expect("N34 configured N33 actual profile");
    assert_eq!(
        prove_common_articulation_pose_authority_v2(fixture.input(&cross_cap))
            .expect_err("decomposition profile binding differs across configured caps"),
        CommonArticulationPoseErrorV2::ResourceLimit,
    );
    let cross_cap_decomposition = fixture.decomposition_with_profile(&cross_cap);
    assert_eq!(
        prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
            decomposition: &cross_cap_decomposition,
            ..fixture.input(&exact)
        })
        .expect_err("configured-N34 decomposition cannot impersonate exact-N33 binding"),
        CommonArticulationPoseErrorV2::ResourceLimit,
    );
    assert!(authority.revalidate_v2(fixture.input(&exact)).is_ok());
}

#[test]
fn equal_total_face_counts_cannot_impersonate_canonical_miura_blocks() {
    assert_eq!(
        validate_canonical_miura_block_shape_v2(10, 12),
        Err(CommonArticulationPoseErrorV2::ResourceLimit),
    );
    assert_eq!(
        validate_canonical_miura_block_shape_v2(8, 12),
        Err(CommonArticulationPoseErrorV2::ResourceLimit),
    );
    assert_eq!(
        validate_canonical_miura_block_shape_v2(9, 11),
        Err(CommonArticulationPoseErrorV2::ResourceLimit),
    );
    assert!(validate_canonical_miura_block_shape_v2(9, 12).is_ok());
}

#[test]
fn revalidation_rejects_foreign_geometry_pose_and_decomposition() {
    let fixture = miura_fixture_v2(33);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33).expect("N33 profile");
    let authority = prove_common_articulation_pose_authority_v2(fixture.input(&profile))
        .expect("N33 authority");

    let foreign = miura_fixture_v2(33);
    assert_eq!(
        authority
            .revalidate_v2(CommonArticulationPoseInputV2 {
                geometry: &foreign.geometry,
                ..fixture.input(&profile)
            })
            .expect_err("foreign geometry"),
        CommonArticulationPoseErrorV2::PoseIssuerMismatch,
    );

    let foreign_pose = fixture.new_pose_instance();
    assert_eq!(
        authority
            .revalidate_v2(CommonArticulationPoseInputV2 {
                pose: &foreign_pose,
                ..fixture.input(&profile)
            })
            .expect_err("foreign pose instance"),
        CommonArticulationPoseErrorV2::IssuerMismatch,
    );

    let foreign_profile = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(34, 33)
        .expect("N34 configured N33 actual profile");
    let foreign_decomposition = fixture.decomposition_with_profile(&foreign_profile);
    assert_eq!(
        authority
            .revalidate_v2(CommonArticulationPoseInputV2 {
                decomposition: &foreign_decomposition,
                profile: &foreign_profile,
                ..fixture.input(&profile)
            })
            .expect_err("foreign decomposition binding"),
        CommonArticulationPoseErrorV2::IssuerMismatch,
    );
}

#[test]
fn issuance_and_revalidation_honor_cancel_and_deadline() {
    let fixture = miura_fixture_v2(33);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33).expect("N33 profile");
    assert_eq!(
        prove_common_articulation_pose_authority_with_checkpoint_v2(
            fixture.input(&profile),
            || { Err(CommonArticulationPoseStopV2::Cancelled) }
        )
        .expect_err("cancelled issuance"),
        CommonArticulationPoseErrorV2::Cancelled,
    );
    assert_eq!(
        prove_common_articulation_pose_authority_with_checkpoint_v2(
            fixture.input(&profile),
            || { Err(CommonArticulationPoseStopV2::DeadlineExceeded) }
        )
        .expect_err("deadline issuance"),
        CommonArticulationPoseErrorV2::DeadlineExceeded,
    );
    let authority = prove_common_articulation_pose_authority_v2(fixture.input(&profile))
        .expect("N33 authority");
    assert_eq!(
        authority
            .revalidate_with_checkpoint_v2(fixture.input(&profile), || {
                Err(CommonArticulationPoseStopV2::Cancelled)
            })
            .expect_err("cancelled revalidation"),
        CommonArticulationPoseErrorV2::Cancelled,
    );
}

#[test]
fn profile_bound_decomposition_honors_start_and_batched_stop_requests() {
    let fixture = miura_fixture_v2(33);
    let profile =
        CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33).expect("N33 profile");
    assert_eq!(
        fixture
            .geometry
            .decompose_canonical_edge_blocks_with_checkpoint_v2(&fixture.audit, &profile, || {
                Err(crate::CommonArticulationDecompositionStopV2::Cancelled)
            })
            .expect_err("start cancellation"),
        crate::CommonArticulationDecompositionErrorV2::Cancelled,
    );
    let mut checkpoints = 0usize;
    assert_eq!(
        fixture
            .geometry
            .decompose_canonical_edge_blocks_with_checkpoint_v2(&fixture.audit, &profile, || {
                checkpoints += 1;
                if checkpoints >= 2 {
                    Err(crate::CommonArticulationDecompositionStopV2::DeadlineExceeded)
                } else {
                    Ok(())
                }
            })
            .expect_err("batched deadline"),
        crate::CommonArticulationDecompositionErrorV2::DeadlineExceeded,
    );

    let mut successful_checkpoints = 0usize;
    fixture
        .geometry
        .decompose_canonical_edge_blocks_with_checkpoint_v2(&fixture.audit, &profile, || {
            successful_checkpoints += 1;
            Ok(())
        })
        .expect("deterministic checkpoint sequence");
    assert!(successful_checkpoints >= 3);
    let mut prepublication_checkpoints = 0usize;
    assert_eq!(
        fixture
            .geometry
            .decompose_canonical_edge_blocks_with_checkpoint_v2(&fixture.audit, &profile, || {
                prepublication_checkpoints += 1;
                if prepublication_checkpoints == successful_checkpoints {
                    Err(crate::CommonArticulationDecompositionStopV2::Cancelled)
                } else {
                    Ok(())
                }
            })
            .expect_err("prepublication cancellation"),
        crate::CommonArticulationDecompositionErrorV2::Cancelled,
    );
}

#[test]
fn v2_decomposition_binds_profile_source_and_canonical_output() {
    let namespace = ProjectId::new();
    let (geometry, audit) = miura_geometry_and_audit_v2(33, namespace);
    let profile = CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(33)
        .expect("exact N33 profile");
    let first = geometry
        .decompose_canonical_edge_blocks_with_profile_v2(&audit, &profile)
        .expect("first N33 decomposition");
    let second = geometry
        .decompose_canonical_edge_blocks_with_profile_v2(&audit, &profile)
        .expect("second N33 decomposition");
    assert_eq!(first.limits().max_blocks, 33);
    assert_eq!(first.limits().max_faces_per_block, 9);
    assert_eq!(first.limits().max_hinges_per_block, 12);
    assert_eq!(first.actual_block_count_v2(), 33);
    assert_eq!(first.face_count_v2(), 265);
    assert_eq!(first.hinge_count_v2(), 396);
    assert_eq!(
        first.logical_work_v2(),
        profile.actual_v2().decomposition_logical_work_v2()
    );
    assert_eq!(
        first.storage_bytes_upper_bound_v2(),
        profile.actual_v2().decomposition_storage_bytes_v2()
    );
    assert_eq!(
        first.profile_binding_fingerprint_v2(),
        profile.binding_fingerprint_v2()
    );
    assert_eq!(
        first.binding_fingerprint_v2(),
        second.binding_fingerprint_v2()
    );
    assert!(first.is_for_geometry(&geometry));
    assert!(first.is_for_profile_v2(&profile));
    let (same_ids_geometry, same_ids_audit) = miura_geometry_and_audit_v2(33, namespace);
    let same_ids = same_ids_geometry
        .decompose_canonical_edge_blocks_with_profile_v2(&same_ids_audit, &profile)
        .expect("independently allocated canonical source");
    assert_eq!(
        first.binding_fingerprint_v2(),
        same_ids.binding_fingerprint_v2()
    );
    assert!(!first.is_for_geometry(&same_ids_geometry));
    assert!(first.blocks().windows(2).all(|pair| {
        let previous = (
            pair[0].geometry().face_ids()[0].canonical_bytes(),
            pair[0].geometry().hinges()[0].edge().canonical_bytes(),
        );
        let next = (
            pair[1].geometry().face_ids()[0].canonical_bytes(),
            pair[1].geometry().hinges()[0].edge().canonical_bytes(),
        );
        previous < next
    }));
    assert!(
        first
            .articulation_faces()
            .windows(2)
            .all(|pair| pair[0].canonical_bytes() < pair[1].canonical_bytes())
    );

    let foreign = miura_fixture_v2(33);
    assert_eq!(
        geometry
            .decompose_canonical_edge_blocks_with_profile_v2(&foreign.audit, &profile)
            .expect_err("foreign audit"),
        crate::CommonArticulationDecompositionErrorV2::InvalidInput,
    );
    let cross_cap = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(34, 33)
        .expect("N34 configured N33 actual profile");
    let cross_cap_decomposition = geometry
        .decompose_canonical_edge_blocks_with_profile_v2(&audit, &cross_cap)
        .expect("cross-cap decomposition");
    assert!(!cross_cap_decomposition.is_for_profile_v2(&profile));
}

#[test]
fn v1_n32_decomposition_contract_remains_available_and_bounded() {
    let (geometry, audit) = miura_geometry_and_audit_v2(32, ProjectId::new());
    let decomposition = geometry
        .decompose_canonical_edge_blocks_v1(
            &audit,
            CanonicalEdgeBlockLimitsV1 {
                max_blocks: 32,
                max_faces_per_block: 9,
                max_hinges_per_block: 12,
            },
        )
        .expect("unchanged V1 N32 decomposition");
    assert_eq!(decomposition.limits().max_blocks, 32);
    assert_eq!(decomposition.blocks().len(), 32);
    assert_eq!(decomposition.articulation_faces().len(), 31);
}

#[test]
fn n64_fixture_ids_and_resource_arithmetic_remain_general_n_safe() {
    let profile = CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(64)
        .expect("N64 general-N resource profile");
    let resources = profile.actual_v2();
    // Independent evaluations: F=8N+1, H=12N, and the checked V2
    // decomposition/pose formulae.  This stays light: no topology or pose
    // solve is needed to prove the fixture's wide-coordinate identity space.
    assert_eq!(resources.block_count_v2(), 64);
    assert_eq!(resources.face_count_v2(), 513);
    assert_eq!(resources.hinge_count_v2(), 768);
    assert_eq!(resources.decomposition_logical_work_v2(), 59_488);
    assert_eq!(resources.decomposition_storage_bytes_v2(), 2_900_992);
    assert_eq!(resources.pose_logical_work_v2(), 44_312);
    assert_eq!(resources.pose_retained_bytes_v2(), 112_112);

    let cells = canonical_miura_cells_v2(64);
    let (pattern, _) = miura_pattern_v2(&cells, ProjectId::new());
    let vertex_ids = pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<HashSet<_>>();
    let edge_ids = pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<HashSet<_>>();
    assert_eq!(vertex_ids.len(), pattern.vertices.len());
    assert_eq!(edge_ids.len(), pattern.edges.len());
}

fn miura_fixture_v2(block_count: usize) -> MiuraFixtureV2 {
    miura_fixture_with_namespace_v2(block_count, ProjectId::new())
}

fn miura_fixture_with_namespace_v2(block_count: usize, namespace: ProjectId) -> MiuraFixtureV2 {
    let (geometry, audit) = miura_geometry_and_audit_v2(block_count, namespace);
    let profile = CommonArticulationResourceProfileV2::exact_canonical_miura_3x3_v2(block_count)
        .expect("canonical Miura V2 profile");
    let decomposition = geometry
        .decompose_canonical_edge_blocks_with_profile_v2(&audit, &profile)
        .expect("N33 canonical decomposition");
    let angles = zero_angles_v2(&geometry);
    let pose = geometry
        .solve_closed(&audit, geometry.face_ids()[0], &angles, 0.0)
        .expect("N33 closed pose");
    assert_eq!(geometry.face_ids().len(), 8 * block_count + 1);
    assert_eq!(geometry.hinges().len(), 12 * block_count);
    assert_eq!(decomposition.blocks().len(), block_count);
    MiuraFixtureV2 {
        geometry,
        audit,
        pose,
        decomposition,
    }
}

fn miura_geometry_and_audit_v2(
    block_count: usize,
    namespace: ProjectId,
) -> (MaterialHingeGraphGeometry, MaterialHingeGraphAudit) {
    let cells = canonical_miura_cells_v2(block_count);
    let (pattern, paper) = miura_pattern_v2(&cells, namespace);
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("N33 Miura topology");
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("N33 Miura geometry");
    let audit = MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
        .expect("N33 Miura audit");
    (geometry, audit)
}

fn canonical_miura_cells_v2(block_count: usize) -> Vec<(i32, i32)> {
    (0..block_count)
        .flat_map(|index| {
            let x = i32::try_from(index)
                .expect("fixture block index fits i32")
                .checked_mul(2)
                .expect("fixture block x fits i32");
            let y = if index % 2 == 0 { 0_i32 } else { -2_i32 };
            let maximum_x = x.checked_add(2).expect("fixture maximum x fits i32");
            let maximum_y = y.checked_add(2).expect("fixture maximum y fits i32");
            (x..=maximum_x).flat_map(move |x| (y..=maximum_y).map(move |y| (x, y)))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn zero_angles_v2(geometry: &MaterialHingeGraphGeometry) -> CanonicalHingeAngles {
    CanonicalHingeAngles::new(
        geometry
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).expect("zero angle"))
            .collect(),
    )
    .expect("canonical zero angles")
}

fn zero_cycle_schedule_entries_v2(
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

fn miura_pattern_v2(cells: &[(i32, i32)], namespace: ProjectId) -> (CreasePattern, Paper) {
    let mut points = BTreeSet::new();
    let mut incidence =
        BTreeMap::<((i32, i32), (i32, i32)), (usize, (i32, i32), (i32, i32))>::new();
    for &(x, y) in cells {
        let next_x = x.checked_add(1).expect("fixture cell x fits i32");
        let next_y = y.checked_add(1).expect("fixture cell y fits i32");
        let corners = [(x, y), (next_x, y), (next_x, next_y), (x, next_y)];
        points.extend(corners);
        for index in 0..4 {
            let start = corners[index];
            let end = corners[(index + 1) % 4];
            let key = if start < end {
                (start, end)
            } else {
                (end, start)
            };
            incidence
                .entry(key)
                .and_modify(|entry| {
                    entry.0 = entry
                        .0
                        .checked_add(1)
                        .expect("fixture edge incidence fits usize");
                })
                .or_insert((1, start, end));
        }
    }
    let vertices = points
        .iter()
        .map(|&(x, y)| Vertex {
            id: vertex_id_for_fixture_v2(namespace, (x, y)),
            position: Point2::new(f64::from(x) * 20.0, f64::from(y) * 20.0),
        })
        .collect::<Vec<_>>();
    let vertex = |point: (i32, i32)| {
        vertices[points
            .iter()
            .position(|candidate| *candidate == point)
            .expect("cell corner")]
        .id
    };
    let edges = incidence
        .iter()
        .map(|(&(first, second), &(count, start, end))| Edge {
            id: edge_id_for_fixture_v2(namespace, first, second),
            start: vertex(start),
            end: vertex(end),
            kind: if count == 1 {
                EdgeKind::Boundary
            } else if first.1 == second.1 {
                EdgeKind::Mountain
            } else if first.1.rem_euclid(2) == 0 {
                EdgeKind::Valley
            } else {
                EdgeKind::Mountain
            },
        })
        .collect::<Vec<_>>();
    let directed = incidence
        .values()
        .filter(|(count, _, _)| *count == 1)
        .map(|(_, start, end)| (*start, *end))
        .collect::<Vec<_>>();
    let mut boundary = vec![directed[0].0];
    while boundary.len() < directed.len() {
        let cursor = *boundary.last().expect("boundary start");
        boundary.push(
            directed
                .iter()
                .find(|(start, _)| *start == cursor)
                .expect("next boundary edge")
                .1,
        );
    }
    let boundary_vertices = boundary.into_iter().map(vertex).collect();
    (
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices,
            thickness_mm: 0.1,
            ..Paper::default()
        },
    )
}

fn vertex_id_for_fixture_v2(namespace: ProjectId, point: (i32, i32)) -> VertexId {
    let mut preimage = [0_u8; 9];
    preimage[0] = 0xc1;
    preimage[1..].copy_from_slice(&fixture_point_bytes_v2(point));
    VertexId::derive_v5(namespace, &preimage)
}

fn edge_id_for_fixture_v2(namespace: ProjectId, first: (i32, i32), second: (i32, i32)) -> EdgeId {
    let mut preimage = [0_u8; 17];
    preimage[0] = 0xc2;
    preimage[1..9].copy_from_slice(&fixture_point_bytes_v2(first));
    preimage[9..].copy_from_slice(&fixture_point_bytes_v2(second));
    EdgeId::derive_v5(namespace, &preimage)
}

fn fixture_point_bytes_v2(point: (i32, i32)) -> [u8; 8] {
    let mut bytes = [0_u8; 8];
    bytes[..4].copy_from_slice(&point.0.to_le_bytes());
    bytes[4..].copy_from_slice(&point.1.to_le_bytes());
    bytes
}
