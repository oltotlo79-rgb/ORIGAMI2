use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex};
use ori_kinematics::{CycleScheduleEntryInputV1, CycleScheduleLimitsV1, TreeKinematicsLimits};
use ori_topology::{FaceExtractionInput, analyze_faces};
use serde::de::DeserializeOwned;

use super::*;

type Fixture = (
    MaterialHingeGraphGeometry,
    MaterialHingeGraphAudit,
    CanonicalCycleScheduleV1,
    FaceId,
);
type Relief = (
    Vec<HingeReliefPolicyRecordV1>,
    Vec<HingeReliefLinearAngleScheduleV1>,
    NativeHingeReliefPrerequisiteV1,
    NativeHingeReliefLocalIntervalCertificateV1,
    HingeReliefPolicyLimitsV1,
);

fn id<T: DeserializeOwned>(prefix: &str, suffix: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-4000-{prefix}-{suffix:012x}\"")).unwrap()
}

/// A production topology with one geometric crease split into two or
/// three collinear edge records. Both material faces therefore share the
/// complete canonical edge list; no test-only graph constructor is used.
fn segmented_crease(hinge_count: usize, revision: u64) -> Fixture {
    assert!((2..=3).contains(&hinge_count));
    let boundary_points = [
        (0.0, 0.0),
        (5.0, 0.0),
        (10.0, 0.0),
        (10.0, 6.0),
        (5.0, 6.0),
        (0.0, 6.0),
    ];
    let mut vertices = boundary_points
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: id("a201", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    vertices.extend((1..hinge_count).map(|index| Vertex {
        id: id("a202", index as u64),
        position: Point2::new(5.0, 6.0 * index as f64 / hinge_count as f64),
    }));
    let boundary = vertices[..6]
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: id("a203", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let mut crease_vertices = vec![boundary[1]];
    crease_vertices.extend(vertices[6..].iter().map(|vertex| vertex.id));
    crease_vertices.push(boundary[4]);
    let mut hinge_edges = Vec::new();
    for index in 0..hinge_count {
        let hinge = id("a204", index as u64 + 1);
        hinge_edges.push(hinge);
        edges.push(Edge {
            id: hinge,
            start: crease_vertices[index],
            end: crease_vertices[index + 1],
            kind: EdgeKind::Mountain,
        });
    }
    let paper = Paper {
        boundary_vertices: boundary,
        thickness_mm: 0.1,
        ..Paper::default()
    };
    let pattern = CreasePattern { vertices, edges };
    let analysis = analyze_faces(FaceExtractionInput {
        identity_namespace: id::<ProjectId>("a205", 1),
        source_revision: revision,
        paper: &paper,
        pattern: &pattern,
    });
    let topology = analysis
        .snapshot
        .unwrap_or_else(|| panic!("segmented crease topology: {:?}", analysis.issues));
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    assert_eq!(geometry.face_ids().len(), 2);
    assert_eq!(geometry.hinges().len(), hinge_count);
    let fixed = audit.faces()[0];
    let mut entries = hinge_edges
        .into_iter()
        .map(|edge| CycleScheduleEntryInputV1 {
            edge,
            initial_angle_degrees_bits: 90.0_f64.to_bits(),
            chebyshev_coefficients: Vec::new(),
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    let schedule = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed,
        [0.0, 1.0],
        entries,
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    (geometry, audit, schedule, fixed)
}

fn relief(
    gaps: &MultiHingeReliefUnionGapReportV2,
    geometry: &MaterialHingeGraphGeometry,
) -> Relief {
    let mut hinges = gaps
        .gaps()
        .iter()
        .flat_map(|gap| gap.hinges())
        .copied()
        .collect::<Vec<_>>();
    hinges.sort_unstable_by_key(|hinge| hinge.hinge().canonical_bytes());
    let policies = hinges
        .iter()
        .map(|hinge| HingeReliefPolicyRecordV1 {
            edge: hinge.hinge(),
            cutout_width_mm: 7.0,
            bevel_angle_degrees: 1.0,
            material_thickness_mm: 0.1,
        })
        .collect::<Vec<_>>();
    let schedules = hinges
        .iter()
        .map(|hinge| HingeReliefLinearAngleScheduleV1 {
            edge: hinge.hinge(),
            source_angle_degrees: f64::from_bits(hinge.source_angle_bits()),
            target_angle_degrees: f64::from_bits(hinge.target_angle_bits()),
        })
        .collect::<Vec<_>>();
    let limits = HingeReliefPolicyLimitsV1::default();
    let prerequisite =
        crate::prepare_hinge_relief_prerequisite_v1(geometry, 0.1, &policies, limits).unwrap();
    let local = crate::certify_hinge_relief_local_intervals_v1(
        &prerequisite,
        geometry,
        0.1,
        &policies,
        &schedules,
        limits,
    )
    .unwrap();
    (policies, schedules, prerequisite, local, limits)
}

#[test]
fn production_two_and_three_segment_creases_certify_the_complete_local_union_only() {
    let mut hashes = Vec::new();
    for hinge_count in [2_usize, 3] {
        let (geometry, audit, schedule, fixed) = segmented_crease(hinge_count, 1);
        let limits = MultiHingeReliefUnionLimitsV2::default();
        let gaps = diagnose_multi_hinge_relief_union_gaps_v2(
            &geometry, &audit, fixed, &schedule, 0.1, limits,
        )
        .unwrap();
        assert_eq!(gaps.gaps().len(), 1);
        assert_eq!(gaps.gaps()[0].hinges().len(), hinge_count);
        assert!(
            gaps.gaps()[0].hinges().windows(2).all(|pair| {
                pair[0].hinge().canonical_bytes() < pair[1].hinge().canonical_bytes()
            })
        );
        assert!(gaps.is_for(&geometry, &audit, fixed, &schedule, 0.1, limits));

        // The old classifier remains fail-closed. V2 does not claim the
        // still-missing union-exterior interval-separation primitive.
        let v1 = crate::diagnose_continuous_pair_coverage_v1(&geometry, &audit, fixed, &schedule)
            .unwrap();
        assert_eq!(
            v1.entries()[0].kind(),
            crate::ContinuousPairCoverageKindV1::Unsupported
        );

        let (policies, schedules, prerequisite, local, policy_limits) = relief(&gaps, &geometry);
        let certificate = certify_multi_hinge_relief_union_v2(
            &gaps,
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            &prerequisite,
            &local,
            &policies,
            &schedules,
            policy_limits,
            limits,
        )
        .unwrap();
        assert_eq!(certificate.covered().len(), 1);
        assert_eq!(certificate.covered()[0].hinges().len(), hinge_count);
        assert!(certificate.covers_every_reported_hinge_neighbourhood());
        assert!(!certificate.authorizes_continuous_motion());
        assert!(!certificate.authorizes_collision_free_classification());
        assert!(!certificate.authorizes_project_mutation());
        assert!(
            revalidate_multi_hinge_relief_union_certificate_v2(
                &certificate,
                &gaps,
                &geometry,
                &audit,
                fixed,
                &schedule,
                0.1,
                &prerequisite,
                &local,
                &policies,
                &schedules,
                policy_limits,
                limits,
            )
            .is_ok()
        );
        hashes.push(certificate.content_hash_v2());
    }
    assert_ne!(hashes[0], hashes[1]);
}

#[test]
fn fresh_revalidation_rejects_missing_duplicate_order_foreign_and_tamper() {
    let (geometry, audit, schedule, fixed) = segmented_crease(3, 1);
    let limits = MultiHingeReliefUnionLimitsV2::default();
    let gaps =
        diagnose_multi_hinge_relief_union_gaps_v2(&geometry, &audit, fixed, &schedule, 0.1, limits)
            .unwrap();
    let (policies, schedules, prerequisite, local, policy_limits) = relief(&gaps, &geometry);
    let certificate = certify_multi_hinge_relief_union_v2(
        &gaps,
        &geometry,
        &audit,
        fixed,
        &schedule,
        0.1,
        &prerequisite,
        &local,
        &policies,
        &schedules,
        policy_limits,
        limits,
    )
    .unwrap();

    let mut missing = gaps.clone();
    missing.gaps[0].hinges.pop();
    assert!(!missing.is_for(&geometry, &audit, fixed, &schedule, 0.1, limits));
    let mut duplicate = gaps.clone();
    duplicate.gaps[0].hinges[1] = duplicate.gaps[0].hinges[0];
    assert!(!duplicate.is_for(&geometry, &audit, fixed, &schedule, 0.1, limits));
    let mut order = gaps.clone();
    order.gaps[0].hinges.swap(0, 1);
    assert!(!order.is_for(&geometry, &audit, fixed, &schedule, 0.1, limits));
    let mut hash = gaps.clone();
    hash.content_hash[0] ^= 1;
    assert!(!hash.is_for(&geometry, &audit, fixed, &schedule, 0.1, limits));
    let mut geometry_binding = gaps.clone();
    geometry_binding.geometry_hash[0] ^= 1;
    assert!(!geometry_binding.is_for(&geometry, &audit, fixed, &schedule, 0.1, limits));
    let mut schedule_binding = gaps.clone();
    schedule_binding.schedule_hash[0] ^= 1;
    assert!(!schedule_binding.is_for(&geometry, &audit, fixed, &schedule, 0.1, limits));
    let mut thickness_binding = gaps.clone();
    thickness_binding.thickness_bits ^= 1;
    assert!(!thickness_binding.is_for(&geometry, &audit, fixed, &schedule, 0.1, limits));

    let (foreign_geometry, foreign_audit, foreign_schedule, foreign_fixed) = segmented_crease(3, 2);
    assert!(!gaps.is_for(
        &foreign_geometry,
        &foreign_audit,
        foreign_fixed,
        &foreign_schedule,
        0.1,
        limits,
    ));
    assert!(
        revalidate_multi_hinge_relief_union_certificate_v2(
            &certificate,
            &gaps,
            &foreign_geometry,
            &foreign_audit,
            foreign_fixed,
            &foreign_schedule,
            0.1,
            &prerequisite,
            &local,
            &policies,
            &schedules,
            policy_limits,
            limits,
        )
        .is_err()
    );

    let mut missing_policies = policies.clone();
    missing_policies.pop();
    let mut missing_schedules = schedules.clone();
    missing_schedules.pop();
    let missing_prerequisite = crate::prepare_hinge_relief_prerequisite_v1(
        &geometry,
        0.1,
        &missing_policies,
        policy_limits,
    )
    .unwrap();
    let missing_local = crate::certify_hinge_relief_local_intervals_v1(
        &missing_prerequisite,
        &geometry,
        0.1,
        &missing_policies,
        &missing_schedules,
        policy_limits,
    )
    .unwrap();
    assert!(matches!(
        certify_multi_hinge_relief_union_v2(
            &gaps,
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            &missing_prerequisite,
            &missing_local,
            &missing_policies,
            &missing_schedules,
            policy_limits,
            limits,
        ),
        Err(MultiHingeReliefUnionErrorV2::IncompleteCoverage)
    ));

    let mut duplicate_policies = policies.clone();
    duplicate_policies[1].edge = duplicate_policies[0].edge;
    assert!(matches!(
        crate::prepare_hinge_relief_prerequisite_v1(
            &geometry,
            0.1,
            &duplicate_policies,
            policy_limits,
        ),
        Err(crate::HingeReliefPolicyErrorV1::DuplicateEdge)
    ));
    let mut ordered = policies.clone();
    ordered.swap(0, 1);
    assert!(matches!(
        crate::prepare_hinge_relief_prerequisite_v1(&geometry, 0.1, &ordered, policy_limits,),
        Err(crate::HingeReliefPolicyErrorV1::NonCanonicalOrder)
    ));

    let mut tampered = certificate.clone();
    tampered.covered[0].hinges.pop();
    assert!(
        revalidate_multi_hinge_relief_union_certificate_v2(
            &tampered,
            &gaps,
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            &prerequisite,
            &local,
            &policies,
            &schedules,
            policy_limits,
            limits,
        )
        .is_err()
    );
    let mut tampered = certificate;
    tampered.content_hash[0] ^= 1;
    assert!(
        revalidate_multi_hinge_relief_union_certificate_v2(
            &tampered,
            &gaps,
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            &prerequisite,
            &local,
            &policies,
            &schedules,
            policy_limits,
            limits,
        )
        .is_err()
    );

    let certificate = certify_multi_hinge_relief_union_v2(
        &gaps,
        &geometry,
        &audit,
        fixed,
        &schedule,
        0.1,
        &prerequisite,
        &local,
        &policies,
        &schedules,
        policy_limits,
        limits,
    )
    .unwrap();
    assert!(
        revalidate_multi_hinge_relief_union_certificate_v2(
            &certificate,
            &gaps,
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            &prerequisite,
            &local,
            &policies,
            &schedules,
            HingeReliefPolicyLimitsV1 {
                max_records: policy_limits.max_records - 1,
            },
            limits,
        )
        .is_err()
    );
}

#[test]
fn work_storage_hinge_caps_and_cancellation_are_exactly_fail_closed() {
    let (geometry, audit, schedule, fixed) = segmented_crease(3, 1);
    let generous = MultiHingeReliefUnionLimitsV2::default();
    let gaps = diagnose_multi_hinge_relief_union_gaps_v2(
        &geometry, &audit, fixed, &schedule, 0.1, generous,
    )
    .unwrap();
    let exact = MultiHingeReliefUnionLimitsV2 {
        max_work: gaps.work_used(),
        max_storage_bytes: gaps.peak_storage_bytes(),
        ..generous
    };
    assert!(
        diagnose_multi_hinge_relief_union_gaps_v2(&geometry, &audit, fixed, &schedule, 0.1, exact,)
            .is_ok()
    );
    for one_short in [
        MultiHingeReliefUnionLimitsV2 {
            max_work: gaps.work_used() - 1,
            ..exact
        },
        MultiHingeReliefUnionLimitsV2 {
            max_storage_bytes: gaps.peak_storage_bytes() - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            diagnose_multi_hinge_relief_union_gaps_v2(
                &geometry, &audit, fixed, &schedule, 0.1, one_short,
            ),
            Err(MultiHingeReliefUnionErrorV2::ResourceLimit)
        ));
    }
    assert!(matches!(
        diagnose_multi_hinge_relief_union_gaps_v2(
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            MultiHingeReliefUnionLimitsV2 {
                max_hinges_per_pair: 2,
                ..generous
            },
        ),
        Err(MultiHingeReliefUnionErrorV2::ResourceLimit)
    ));
    let mut checkpoints = 0;
    assert!(matches!(
        diagnose_multi_hinge_relief_union_gaps_with_cancel_v2(
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            generous,
            || {
                checkpoints += 1;
                checkpoints > 2
            },
        ),
        Err(MultiHingeReliefUnionErrorV2::Cancelled)
    ));

    let (policies, schedules, prerequisite, local, policy_limits) = relief(&gaps, &geometry);
    let certificate = certify_multi_hinge_relief_union_v2(
        &gaps,
        &geometry,
        &audit,
        fixed,
        &schedule,
        0.1,
        &prerequisite,
        &local,
        &policies,
        &schedules,
        policy_limits,
        generous,
    )
    .unwrap();
    let exact = MultiHingeReliefUnionLimitsV2 {
        max_work: certificate.work_used(),
        max_storage_bytes: certificate.peak_storage_bytes(),
        ..generous
    };
    assert!(
        certify_multi_hinge_relief_union_v2(
            &gaps,
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            &prerequisite,
            &local,
            &policies,
            &schedules,
            policy_limits,
            exact,
        )
        .is_ok()
    );
    for one_short in [
        MultiHingeReliefUnionLimitsV2 {
            max_work: certificate.work_used() - 1,
            ..exact
        },
        MultiHingeReliefUnionLimitsV2 {
            max_storage_bytes: certificate.peak_storage_bytes() - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            certify_multi_hinge_relief_union_v2(
                &gaps,
                &geometry,
                &audit,
                fixed,
                &schedule,
                0.1,
                &prerequisite,
                &local,
                &policies,
                &schedules,
                policy_limits,
                one_short,
            ),
            Err(MultiHingeReliefUnionErrorV2::ResourceLimit)
        ));
    }
    let mut checkpoints = 0;
    assert!(matches!(
        certify_multi_hinge_relief_union_with_cancel_v2(
            &gaps,
            &geometry,
            &audit,
            fixed,
            &schedule,
            0.1,
            &prerequisite,
            &local,
            &policies,
            &schedules,
            policy_limits,
            generous,
            || {
                checkpoints += 1;
                checkpoints > 3
            },
        ),
        Err(MultiHingeReliefUnionErrorV2::Cancelled)
    ));
}
