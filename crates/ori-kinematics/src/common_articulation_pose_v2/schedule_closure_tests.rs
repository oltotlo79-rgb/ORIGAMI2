//! Schedule restriction and closure evidence for V2 general-N blocks.

use std::collections::HashSet;

use super::*;
use crate::{
    CanonicalCycleScheduleV1, CycleScheduleLimitsV1, CycleSchedulePrepareErrorV1,
    CycleScheduleRestrictionErrorV1, CycleScheduleRestrictionStopV1,
    DyadicIntervalClosureControlErrorV1, DyadicIntervalClosureErrorV1,
    DyadicIntervalClosureLimitsV1, DyadicIntervalClosureStopV1,
};

#[test]
fn controlled_dyadic_closure_preserves_v1_result_and_hides_partial_stop() {
    let fixture = miura_fixture_v2(34);
    let parent_fixed_face = fixture.geometry.face_ids()[0];
    let schedule = CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        parent_fixed_face,
        [0.0, 1.0],
        zero_cycle_schedule_entries_v2(&fixture.geometry),
        parent_schedule_limits_v2(),
    )
    .expect("N34 parent schedule");
    let block_fixed_face = block_articulation_face_v2(&fixture, 0);
    let restricted = restrict_block_v2(&fixture, &schedule, 0, block_fixed_face)
        .expect("first block restriction");
    let block = &fixture.decomposition.blocks()[0];
    let baseline = block
        .geometry()
        .prove_dyadic_schedule_closure_v1(
            block.audit(),
            block_fixed_face,
            &restricted,
            0.0,
            block_closure_limits_v2(),
        )
        .expect("legacy no-op checkpoint path");
    let controlled = block
        .geometry()
        .prove_dyadic_schedule_closure_with_checkpoint_v1(
            block.audit(),
            block_fixed_face,
            &restricted,
            0.0,
            block_closure_limits_v2(),
            || Ok(()),
        )
        .expect("controlled no-op checkpoint path");
    assert_eq!(controlled, baseline);

    let mut polls = 0usize;
    assert_eq!(
        block
            .geometry()
            .prove_dyadic_schedule_closure_with_checkpoint_v1(
                block.audit(),
                block_fixed_face,
                &restricted,
                0.0,
                block_closure_limits_v2(),
                || {
                    polls += 1;
                    (polls != 3)
                        .then_some(())
                        .ok_or(DyadicIntervalClosureStopV1::Cancelled)
                },
            )
            .expect_err("stop before a closure certificate is published"),
        DyadicIntervalClosureControlErrorV1::Cancelled,
    );
    assert_eq!(polls, 3);
}

#[test]
fn n34_v2_blocks_support_complete_v1_schedule_restriction_and_closure() {
    const N34_BLOCKS: usize = 34;
    const N34_HINGES: usize = 408;
    const BLOCK_HINGES: usize = 12;
    const MID_BLOCK: usize = N34_BLOCKS / 2;

    let fixture = miura_fixture_v2(N34_BLOCKS);
    let fixed_face = fixture.geometry.face_ids()[0];
    let entries = zero_cycle_schedule_entries_v2(&fixture.geometry);
    let schedule = CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixed_face,
        [0.0, 1.0],
        entries.clone(),
        parent_schedule_limits_v2(),
    )
    .expect("N34 parent schedule");
    let closure_limits = block_closure_limits_v2();

    let mut covered_parent_edges = HashSet::new();
    for (block_index, block) in fixture.decomposition.blocks().iter().enumerate() {
        let block_fixed_face = block_articulation_face_v2(&fixture, block_index);
        let restricted = restrict_block_v2(&fixture, &schedule, block_index, block_fixed_face)
            .expect("V2 block exposes a valid V1 geometry/audit carrier");
        assert!(restricted.matches_binding(block.geometry(), block.audit(), block_fixed_face));
        let restricted_angles = restricted
            .try_evaluate_v1(0.0)
            .expect("restricted zero schedule");
        let restricted_edges = restricted_angles
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
        assert_eq!(restricted_angles.as_slice().len(), BLOCK_HINGES);
        assert_eq!(restricted_edges, block_edges, "block {block_index}");
        covered_parent_edges.extend(block_edges);

        let closure = block
            .geometry()
            .prove_dyadic_schedule_closure_v1(
                block.audit(),
                block_fixed_face,
                &restricted,
                0.0,
                closure_limits,
            )
            .expect("stationary restricted block has one bounded closure leaf");
        assert_eq!(closure.fixed_face(), block_fixed_face);
        assert_eq!(closure.leaves().len(), 1);
        assert!(closure.has_canonical_complete_partition_v1());
        assert!(closure.every_leaf_covers_graph_v1(block.geometry()));
        let reissued = block
            .geometry()
            .prove_dyadic_schedule_closure_v1(
                block.audit(),
                block_fixed_face,
                &restricted,
                0.0,
                closure_limits,
            )
            .expect("same restricted block closure revalidation by deterministic reissue");
        assert_eq!(closure, reissued);
    }
    assert_eq!(fixture.decomposition.blocks().len(), N34_BLOCKS);
    assert_eq!(covered_parent_edges.len(), N34_HINGES);

    let omitted_block_edges = fixture.decomposition.blocks()[MID_BLOCK]
        .geometry()
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<HashSet<_>>();
    let missing_one_block = entries
        .iter()
        .filter(|entry| !omitted_block_edges.contains(&entry.edge))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(missing_one_block.len(), N34_HINGES - BLOCK_HINGES);
    assert_eq!(
        CanonicalCycleScheduleV1::prepare(
            &fixture.geometry,
            &fixture.audit,
            fixed_face,
            [0.0, 1.0],
            missing_one_block,
            parent_schedule_limits_v2(),
        )
        .expect_err("a full-geometry V1 schedule cannot omit one V2 block"),
        CycleSchedulePrepareErrorV1::InvalidInput,
    );

    let foreign_fixture = miura_fixture_v2(N34_BLOCKS);
    let foreign_block = &foreign_fixture.decomposition.blocks()[MID_BLOCK];
    assert_eq!(
        schedule
            .restrict_to_edge_block_with_fixed_face_with_checkpoint_v1(
                &fixture.geometry,
                &fixture.audit,
                foreign_block.geometry(),
                foreign_block.audit(),
                foreign_block.geometry().face_ids()[0],
                || Ok(()),
            )
            .expect_err("foreign V2 block cannot enter the parent schedule"),
        CycleScheduleRestrictionErrorV1::Prepare(CycleSchedulePrepareErrorV1::InvalidInput),
    );

    let first_fixed_face = block_articulation_face_v2(&fixture, 0);
    let first_restricted = restrict_block_v2(&fixture, &schedule, 0, first_fixed_face)
        .expect("first block restriction");
    for one_short in [
        DyadicIntervalClosureLimitsV1 {
            max_leaves: 0,
            ..closure_limits
        },
        DyadicIntervalClosureLimitsV1 {
            max_work: 0,
            ..closure_limits
        },
    ] {
        assert_eq!(
            fixture.decomposition.blocks()[0]
                .geometry()
                .prove_dyadic_schedule_closure_v1(
                    fixture.decomposition.blocks()[0].audit(),
                    first_fixed_face,
                    &first_restricted,
                    0.0,
                    one_short,
                )
                .expect_err("one-short closure limit"),
            DyadicIntervalClosureErrorV1::InvalidInput,
        );
    }

    let mut cancellation_polls = 0usize;
    assert_eq!(
        restrict_block_with_checkpoint_v2(&fixture, &schedule, 0, first_fixed_face, || {
            cancellation_polls += 1;
            (cancellation_polls != 64)
                .then_some(())
                .ok_or(CycleScheduleRestrictionStopV1::Cancelled)
        })
        .expect_err("mid-restriction cancellation"),
        CycleScheduleRestrictionErrorV1::Cancelled,
    );
    assert_eq!(cancellation_polls, 64);
    let mut deadline_polls = 0usize;
    assert_eq!(
        restrict_block_with_checkpoint_v2(&fixture, &schedule, 0, first_fixed_face, || {
            deadline_polls += 1;
            (deadline_polls != 65)
                .then_some(())
                .ok_or(CycleScheduleRestrictionStopV1::DeadlineExceeded)
        })
        .expect_err("mid-restriction deadline"),
        CycleScheduleRestrictionErrorV1::DeadlineExceeded,
    );
    assert_eq!(deadline_polls, 65);
}

fn parent_schedule_limits_v2() -> CycleScheduleLimitsV1 {
    CycleScheduleLimitsV1 {
        max_hinges: 408,
        max_degree: 0,
        max_coefficient_bits: 1,
        max_work: 408,
    }
}

fn block_closure_limits_v2() -> DyadicIntervalClosureLimitsV1 {
    DyadicIntervalClosureLimitsV1 {
        max_depth: 0,
        max_leaves: 1,
        max_work: 1,
        schedule_limits: CycleScheduleLimitsV1 {
            max_hinges: 12,
            max_degree: 0,
            max_coefficient_bits: 1,
            max_work: 12,
        },
    }
}

fn block_articulation_face_v2(fixture: &MiuraFixtureV2, block_index: usize) -> ori_domain::FaceId {
    fixture.decomposition.blocks()[block_index]
        .geometry()
        .face_ids()
        .iter()
        .copied()
        .find(|face| fixture.decomposition.articulation_faces().contains(face))
        .expect("every block in the N34 chain has an articulation face")
}

fn restrict_block_v2(
    fixture: &MiuraFixtureV2,
    schedule: &CanonicalCycleScheduleV1,
    block_index: usize,
    fixed_face: ori_domain::FaceId,
) -> Result<CanonicalCycleScheduleV1, CycleScheduleRestrictionErrorV1> {
    restrict_block_with_checkpoint_v2(fixture, schedule, block_index, fixed_face, || Ok(()))
}

fn restrict_block_with_checkpoint_v2(
    fixture: &MiuraFixtureV2,
    schedule: &CanonicalCycleScheduleV1,
    block_index: usize,
    fixed_face: ori_domain::FaceId,
    checkpoint: impl FnMut() -> Result<(), CycleScheduleRestrictionStopV1>,
) -> Result<CanonicalCycleScheduleV1, CycleScheduleRestrictionErrorV1> {
    let block = &fixture.decomposition.blocks()[block_index];
    schedule.restrict_to_edge_block_with_fixed_face_with_checkpoint_v1(
        &fixture.geometry,
        &fixture.audit,
        block.geometry(),
        block.audit(),
        fixed_face,
        checkpoint,
    )
}
