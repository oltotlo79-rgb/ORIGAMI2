//! Regression coverage for the native four-cycle cactus path.  This remains
//! deliberately separate from desktop post-Apply proof tests: the authority
//! here is issued directly by the collision/kinematics boundary.

use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, Instant},
};

use ori_kinematics::{CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, TreeKinematicsLimits};
use ori_topology::{FaceExtractionInput, analyze_faces};

use super::*;

fn four_cycle_geometry_at_revision_v1(
    revision: u64,
) -> (MaterialHingeGraphGeometry, MaterialHingeGraphAudit, FaceId) {
    let (pattern, paper, _) =
        super::super::four_bay_cycle_test_support::four_bay_rational_cycle_pattern();
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id("b600", 1),
        source_revision: revision,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("four-cycle rational cactus topology");
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("revision-bound geometry");
    let fixed = topology
        .faces
        .iter()
        .max_by_key(|face| {
            topology
                .hinge_adjacency
                .iter()
                .filter(|hinge| hinge.first == face.id || hinge.second == face.id)
                .count()
        })
        .expect("articulation face")
        .id;
    let audit = MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
        .expect("revision-bound graph audit");
    (geometry, audit, fixed)
}

fn stationary_zero_schedule_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed: FaceId,
) -> ori_kinematics::CanonicalCycleScheduleV1 {
    let mut entries = geometry
        .hinges()
        .iter()
        .map(|hinge| ori_kinematics::CycleScheduleEntryInputV1 {
            edge: hinge.edge(),
            initial_angle_degrees_bits: 0.0_f64.to_bits(),
            chebyshev_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                numerator: 0,
                denominator: 1,
            }],
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    ori_kinematics::CanonicalCycleScheduleV1::prepare(
        geometry,
        audit,
        fixed,
        [0.0, 1.0],
        entries,
        CycleScheduleLimitsV1::default(),
    )
    .expect("canonical stationary schedule")
}

fn stationary_four_cycle_authority_fixture_v1() -> (
    MaterialHingeGraphGeometry,
    MaterialHingeGraphAudit,
    ori_kinematics::CanonicalCycleScheduleV1,
    DyadicMaterialHingeIntervalClosureCertificateV1,
    FaceId,
) {
    let (geometry, audit, _, fixed) = rational_cycle_bay_geometry(4, false);
    let schedule = stationary_zero_schedule_v1(&geometry, &audit, fixed);
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            &audit,
            fixed,
            &schedule,
            1.0e-8,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 0,
                max_leaves: 1,
                max_work: 1,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        )
        .expect("single-leaf stationary multi-cycle closure");
    (geometry, audit, schedule, closure, fixed)
}

#[test]
fn controlled_group_mid_stop_never_mints_or_contaminates_authority() {
    let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(4, false);
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            &audit,
            fixed,
            &schedule,
            1.0e-8,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 2,
                max_leaves: 4,
                max_work: 4,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        )
        .expect("four-leaf closure");

    let stop = configure_controlled_closure_leaf_test_stop_v1(
        2,
        crate::CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled,
    );
    assert!(matches!(
        certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            0.1,
            32,
            &crate::CooperativeOperationControlV1::unbounded(),
        ),
        Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled)
    ));
    drop(stop);

    let stop = configure_controlled_closure_leaf_test_stop_v1(
        3,
        crate::CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded,
    );
    assert!(matches!(
        certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            0.1,
            32,
            &crate::CooperativeOperationControlV1::unbounded(),
        ),
        Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded)
    ));
    drop(stop);

    let stop = configure_controlled_group_test_stop_v1(
        3,
        crate::CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled,
    );
    assert!(matches!(
        certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            0.1,
            32,
            &crate::CooperativeOperationControlV1::unbounded(),
        ),
        Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled)
    ));
    drop(stop);

    let stop = configure_controlled_group_test_stop_v1(
        4,
        crate::CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded,
    );
    assert!(matches!(
        certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            0.1,
            32,
            &crate::CooperativeOperationControlV1::unbounded(),
        ),
        Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded)
    ));
    drop(stop);

    assert!(
        certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            0.1,
            32,
            &crate::CooperativeOperationControlV1::unbounded(),
        )
        .expect("clean control context")
        .is_none(),
        "group separation and static samples alone must not mint continuous authority"
    );
}

#[test]
fn four_cycle_positive_authority_rejects_foreign_revision_cancelled_and_aba_contexts() {
    let (geometry, audit, schedule, fixed) = rational_cycle_bay_geometry(4, false);
    assert_eq!(
        (geometry.face_ids().len(), geometry.hinges().len()),
        (13, 16)
    );
    assert_eq!(
        geometry.hinges().len() + 1 - geometry.face_ids().len(),
        4,
        "fixture must exercise four independent cycle closures"
    );
    assert_eq!(audit.closure_hinges().len(), 4);
    let cycle_groups =
        composed_symmetric_rational_local_groups_v1(&geometry, &audit, fixed, &schedule)
            .expect("four individually recognised symmetric cycle groups");
    let mut group_sizes = cycle_groups
        .values()
        .copied()
        .fold([0_usize; 4], |mut sizes, group| {
            sizes[group] += 1;
            sizes
        });
    group_sizes.sort_unstable();
    assert_eq!(group_sizes, [3, 3, 3, 3]);
    let bounded_start = Instant::now();
    assert!(
        matches!(
            geometry.prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-8,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 2,
                    max_leaves: 3,
                    max_work: 3,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            ),
            Err(ori_kinematics::DyadicIntervalClosureErrorV1::ResourceLimit)
        ),
        "one-short multi-cycle path budget must fail before an authority can be issued"
    );
    assert!(
        bounded_start.elapsed() < Duration::from_secs(2),
        "one-short multi-cycle closure must fail within the worker budget"
    );
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            &audit,
            fixed,
            &schedule,
            1.0e-8,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 2,
                max_leaves: 4,
                max_work: 4,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        )
        .expect("four-leaf multi-cycle closure");
    assert_eq!(closure.leaves().len(), 4);
    let control_start = Instant::now();
    let active = AtomicBool::new(false);
    assert!(
        matches!(
            certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
                &geometry,
                &audit,
                fixed,
                &schedule,
                &closure,
                0.1,
                32,
                &crate::CooperativeOperationControlV1::new(Some(&active), Instant::now()),
            ),
            Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::DeadlineExceeded)
        ),
        "an elapsed deadline must stop the multi-cycle issuer before authority minting"
    );
    let cancelled = AtomicBool::new(true);
    assert!(
        matches!(
            certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
                &geometry,
                &audit,
                fixed,
                &schedule,
                &closure,
                0.1,
                32,
                &crate::CooperativeOperationControlV1::new(
                    Some(&cancelled),
                    Instant::now() + Duration::from_secs(1),
                ),
            ),
            Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled)
        ),
        "a cancellation observed while issuing must leave no partial authority"
    );
    let generation = AtomicU64::new(41);
    let old_generation = crate::CooperativeOperationControlV1::new_with_generation(
        Some(&active),
        &generation,
        41,
        Instant::now() + Duration::from_secs(1),
    );
    generation.store(42, Ordering::Release);
    assert!(
        matches!(
            certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
                &geometry,
                &audit,
                fixed,
                &schedule,
                &closure,
                0.1,
                32,
                &old_generation,
            ),
            Err(crate::CanonicalPositiveThicknessCyclePathControlErrorV1::Cancelled)
        ),
        "an old generation cannot publish after a replacement begins"
    );
    let current_generation = crate::CooperativeOperationControlV1::new_with_generation(
        Some(&active),
        &generation,
        42,
        Instant::now() + Duration::from_secs(1),
    );
    assert!(
        certify_canonical_positive_thickness_cycle_schedule_path_with_control_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            0.1,
            32,
            &current_generation,
        )
        .expect("current generation control")
        .is_none(),
        "finite cactus samples and local group separation are not a continuous proof"
    );
    assert!(
        control_start.elapsed() < Duration::from_secs(2),
        "deadline, cancellation, and ABA issuance checks must remain bounded"
    );

    let (stationary, stationary_audit, stationary_schedule, stationary_closure, stationary_fixed) =
        stationary_four_cycle_authority_fixture_v1();
    let authority = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &stationary,
        &stationary_audit,
        stationary_fixed,
        &stationary_schedule,
        &stationary_closure,
        0.1,
        1,
    )
    .expect("stationary all-pair positive-thickness authority");
    assert!(authority.is_for(
        &stationary,
        &stationary_audit,
        stationary_fixed,
        &stationary_schedule,
        &stationary_closure,
        0.1
    ));

    assert!(
        certify_canonical_positive_thickness_cycle_schedule_path_v1(
            &stationary,
            &stationary_audit,
            stationary_fixed,
            &stationary_schedule,
            &stationary_closure,
            0.1,
            0,
        )
        .is_none(),
        "a zero proof-leaf budget cannot mint authority"
    );
    assert!(
        certify_canonical_positive_thickness_cycle_schedule_path_v1(
            &stationary,
            &stationary_audit,
            stationary_fixed,
            &stationary_schedule,
            &stationary_closure,
            0.1,
            MAX_STACKED_FOLD_INTERVAL_LEAVES_V1 + 1,
        )
        .is_none(),
        "an excessive proof-leaf budget cannot mint authority"
    );

    let wrong_fixed = stationary
        .face_ids()
        .iter()
        .copied()
        .find(|face| *face != stationary_fixed)
        .expect("another face");
    assert!(
        !authority.is_for(
            &stationary,
            &stationary_audit,
            wrong_fixed,
            &stationary_schedule,
            &stationary_closure,
            0.1,
        ),
        "the authority remains fixed-face bound"
    );
    assert!(
        !authority.is_for(
            &stationary,
            &stationary_audit,
            stationary_fixed,
            &stationary_schedule,
            &stationary_closure,
            f64::from_bits(0.1_f64.to_bits() + 1),
        ),
        "the authority remains binary64-thickness bound"
    );

    let (detached, detached_audit, detached_schedule, detached_closure, detached_fixed) =
        stationary_four_cycle_authority_fixture_v1();
    assert!(!stationary.same_instance(&detached));
    assert_eq!(
        stationary_schedule.certificate_binding_fingerprint_v2(),
        detached_schedule.certificate_binding_fingerprint_v2()
    );
    assert_eq!(
        stationary_closure.partition_binding_fingerprint_v2(),
        detached_closure.partition_binding_fingerprint_v2()
    );
    assert!(
        !authority.is_for(
            &stationary,
            &detached_audit,
            stationary_fixed,
            &detached_schedule,
            &detached_closure,
            0.1,
        ),
        "same-content foreign closure evidence must fail the issuer check"
    );
    assert!(
        !authority.is_for(
            &detached,
            &detached_audit,
            detached_fixed,
            &detached_schedule,
            &detached_closure,
            0.1,
        ),
        "same-content replacement geometry must fail the issuer check"
    );
    assert!(
        certify_canonical_positive_thickness_cycle_schedule_path_v1(
            &stationary,
            &stationary_audit,
            stationary_fixed,
            &stationary_schedule,
            &detached_closure,
            0.1,
            1,
        )
        .is_none(),
        "a same-content foreign closure cannot mint replacement authority"
    );

    let (revised, revised_audit, revised_fixed) = four_cycle_geometry_at_revision_v1(2);
    assert!(
        !authority.is_for(
            &revised,
            &revised_audit,
            revised_fixed,
            &stationary_schedule,
            &stationary_closure,
            0.1,
        ),
        "a new source revision must not reuse the old issuer authority"
    );

    assert!(
        authority.is_for(
            &stationary,
            &stationary_audit,
            stationary_fixed,
            &stationary_schedule,
            &stationary_closure,
            0.1,
        ),
        "failed control contexts neither mint a replacement authority nor mutate the issued scene evidence"
    );
}
