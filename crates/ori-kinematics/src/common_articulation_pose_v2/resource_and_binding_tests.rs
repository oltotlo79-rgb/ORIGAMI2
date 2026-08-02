use super::*;

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
