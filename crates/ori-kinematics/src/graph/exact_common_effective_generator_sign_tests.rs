use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::FoldAssignment;

use super::*;
use crate::{
    CycleScheduleEntryInputV1, CycleScheduleLimitsV1, Point3, RationalCoefficientV1, TreeHinge,
};

struct Fixture {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    fixed_face: FaceId,
    schedule: CanonicalCycleScheduleV1,
    profile: ExactCommonLinearCycleProfileV1,
}

fn schedule_entries(edges: &[EdgeId]) -> Vec<CycleScheduleEntryInputV1> {
    edges
        .iter()
        .map(|edge| CycleScheduleEntryInputV1 {
            edge: *edge,
            initial_angle_degrees_bits: 90.0_f64.to_bits(),
            chebyshev_coefficients: vec![
                RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 10,
                    denominator: 1,
                },
            ],
        })
        .collect()
}

fn tree_fixture(
    edge_count: usize,
    fixed_index: usize,
    offset_last: bool,
    mixed_assignments: bool,
) -> Fixture {
    let namespace = ProjectId::schema_namespace([0x71; 16]);
    let faces = (0..=edge_count)
        .map(|index| FaceId::derive_v5(namespace, format!("face-{index}").as_bytes()))
        .collect::<Vec<_>>();
    let edges = (0..edge_count)
        .map(|index| EdgeId::derive_v5(namespace, format!("edge-{index}").as_bytes()))
        .collect::<Vec<_>>();
    let mut hinges = Vec::new();
    for index in 0..edge_count {
        let y = if offset_last && index + 1 == edge_count {
            f64::from_bits(1)
        } else {
            0.0
        };
        let base = 2.0 * index as f64;
        let reversed = mixed_assignments && index % 2 == 1;
        let (assignment, start_x, end_x, axis_x) = if reversed {
            (FoldAssignment::Valley, base + 1.0, base, -1.0)
        } else {
            (FoldAssignment::Mountain, base, base + 1.0, 1.0)
        };
        hinges.push(TreeHinge::new_for_test(
            edges[index],
            assignment,
            faces[index],
            faces[index + 1],
            Point3::new(start_x, y, 0.0).unwrap(),
            Point3::new(end_x, y, 0.0).unwrap(),
            Point3::new(axis_x, 0.0, 0.0).unwrap(),
        ));
    }
    hinges.sort_unstable_by_key(|hinge| hinge.edge().canonical_bytes());
    let mut canonical_faces = faces.clone();
    canonical_faces.sort_unstable_by_key(FaceId::canonical_bytes);
    let mut canonical_edges = edges;
    canonical_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let audit = MaterialHingeGraphAudit {
        faces: canonical_faces.clone(),
        spanning_hinges: canonical_edges.clone(),
        closure_hinges: Vec::new(),
    };
    let geometry = MaterialHingeGraphGeometry::new_for_test(canonical_faces, hinges);
    let fixed_face = faces[fixed_index];
    let schedule = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed_face,
        [0.0, 1.0],
        schedule_entries(&canonical_edges),
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    let profile = schedule
        .prove_exact_common_linear_profile_v1(
            &canonical_edges,
            ExactCommonLinearCycleProfileLimitsV1::default(),
        )
        .unwrap();
    Fixture {
        geometry,
        audit,
        fixed_face,
        schedule,
        profile,
    }
}

fn cycle_fixture() -> Fixture {
    let namespace = ProjectId::schema_namespace([0x72; 16]);
    let faces = (0..3)
        .map(|index| FaceId::derive_v5(namespace, format!("cycle-face-{index}").as_bytes()))
        .collect::<Vec<_>>();
    let edges = (0..3)
        .map(|index| EdgeId::derive_v5(namespace, format!("cycle-edge-{index}").as_bytes()))
        .collect::<Vec<_>>();
    let pairs = [(0usize, 1usize), (1, 2), (2, 0)];
    let mut hinges = pairs
        .iter()
        .enumerate()
        .map(|(index, &(left, right))| {
            TreeHinge::new_for_test(
                edges[index],
                FoldAssignment::Mountain,
                faces[left],
                faces[right],
                Point3::new(index as f64, 0.0, 0.0).unwrap(),
                Point3::new(index as f64 + 1.0, 0.0, 0.0).unwrap(),
                Point3::new(1.0, 0.0, 0.0).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    hinges.sort_unstable_by_key(|hinge| hinge.edge().canonical_bytes());
    let mut canonical_faces = faces.clone();
    canonical_faces.sort_unstable_by_key(FaceId::canonical_bytes);
    let mut canonical_edges = edges;
    canonical_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let audit = MaterialHingeGraphAudit {
        faces: canonical_faces.clone(),
        spanning_hinges: canonical_edges[..2].to_vec(),
        closure_hinges: canonical_edges[2..].to_vec(),
    };
    let geometry = MaterialHingeGraphGeometry::new_for_test(canonical_faces, hinges);
    let fixed_face = faces[0];
    let schedule = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed_face,
        [0.0, 1.0],
        schedule_entries(&canonical_edges),
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    let profile = schedule
        .prove_exact_common_linear_profile_v1(
            &canonical_edges,
            ExactCommonLinearCycleProfileLimitsV1::default(),
        )
        .unwrap();
    Fixture {
        geometry,
        audit,
        fixed_face,
        schedule,
        profile,
    }
}

fn prove(
    fixture: &Fixture,
) -> Result<ExactCommonEffectiveGeneratorSignV1, ExactCommonEffectiveGeneratorSignErrorV1> {
    prove_exact_common_effective_generator_sign_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &fixture.schedule,
        &fixture.profile,
        ExactCommonEffectiveGeneratorSignLimitsV1::default(),
    )
}

#[test]
fn accepts_complete_two_and_three_edge_rooted_carriers() {
    for edge_count in [2, 3] {
        let forward = tree_fixture(edge_count, 0, false, false);
        let proof = prove(&forward).unwrap();
        assert_eq!(
            proof.common_effective_sign(),
            EffectiveGeneratorSignV1::Positive
        );
        assert_eq!(proof.edge_ids(), forward.profile.edge_ids());
        assert_eq!(proof.fixed_face(), forward.fixed_face);
        assert!(!proof.authorizes_closure());
        assert!(!proof.authorizes_collision_clearance());
        assert!(!proof.authorizes_project_mutation());
        proof
            .revalidate_issuers_v1(
                &forward.geometry,
                &forward.audit,
                forward.fixed_face,
                &forward.schedule,
                &forward.profile,
                ExactCommonEffectiveGeneratorSignLimitsV1::default(),
            )
            .unwrap();

        let reverse = tree_fixture(edge_count, edge_count, false, false);
        assert_eq!(
            prove(&reverse).unwrap().common_effective_sign(),
            EffectiveGeneratorSignV1::Negative
        );
    }
}

#[test]
fn effective_sign_is_not_raw_assignment_sign() {
    let mixed = tree_fixture(3, 0, false, true);
    assert_eq!(
        prove(&mixed).unwrap().common_effective_sign(),
        EffectiveGeneratorSignV1::Positive
    );
}

#[test]
fn rejects_offset_sign_mismatch_and_every_cycle() {
    let offset = tree_fixture(3, 0, true, false);
    assert_eq!(
        prove(&offset),
        Err(ExactCommonEffectiveGeneratorSignErrorV1::NonCollinearCarrier)
    );

    let root_in_middle = tree_fixture(2, 1, false, false);
    assert_eq!(
        prove(&root_in_middle),
        Err(ExactCommonEffectiveGeneratorSignErrorV1::EffectiveSignMismatch)
    );

    let cycle = cycle_fixture();
    assert_eq!(
        prove(&cycle),
        Err(ExactCommonEffectiveGeneratorSignErrorV1::UnsupportedRootedCarrier)
    );
}

#[test]
fn issuer_revalidation_rejects_tampering_and_cross_binding() {
    let fixture = tree_fixture(3, 0, false, false);
    let proof = prove(&fixture).unwrap();
    let mut forged = proof.clone();
    forged.proof_fingerprint_v1[0] ^= 1;
    assert_eq!(
        forged.revalidate_issuers_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.profile,
            ExactCommonEffectiveGeneratorSignLimitsV1::default(),
        ),
        Err(ExactCommonEffectiveGeneratorSignErrorV1::IssuerMismatch)
    );

    let other_binding = tree_fixture(3, 3, false, false);
    assert_eq!(
        prove_exact_common_effective_generator_sign_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &other_binding.profile,
            ExactCommonEffectiveGeneratorSignLimitsV1::default(),
        ),
        Err(ExactCommonEffectiveGeneratorSignErrorV1::ProfileIssuerMismatch)
    );
    assert_eq!(
        prove_exact_common_effective_generator_sign_v1(
            &fixture.geometry,
            &fixture.audit,
            other_binding.fixed_face,
            &fixture.schedule,
            &fixture.profile,
            ExactCommonEffectiveGeneratorSignLimitsV1::default(),
        ),
        Err(ExactCommonEffectiveGeneratorSignErrorV1::GraphBindingMismatch)
    );
}

#[test]
fn limits_succeed_at_equality_and_fail_one_short() {
    let fixture = tree_fixture(3, 0, false, false);
    let unbounded = ExactCommonEffectiveGeneratorSignLimitsV1 {
        profile_limits: ExactCommonLinearCycleProfileLimitsV1::default(),
        max_edges: MAX_EDGES_V1,
        max_faces: MAX_FACES_V1,
        max_work: usize::MAX,
        max_retained_bytes: usize::MAX,
        max_peak_bytes: usize::MAX,
    };
    let mut audit_meter = MeterV1::new(unbounded);
    prove_with_meter_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &fixture.schedule,
        &fixture.profile,
        &mut audit_meter,
    )
    .unwrap();
    let exact = ExactCommonEffectiveGeneratorSignLimitsV1 {
        profile_limits: ExactCommonLinearCycleProfileLimitsV1::default(),
        max_edges: fixture.profile.edge_ids().len(),
        max_faces: fixture.geometry.face_ids().len(),
        max_work: audit_meter.work,
        max_retained_bytes: audit_meter.retained_bytes,
        max_peak_bytes: audit_meter.peak_bytes,
    };
    assert!(
        prove_exact_common_effective_generator_sign_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.profile,
            exact,
        )
        .is_ok()
    );

    for one_short in [
        ExactCommonEffectiveGeneratorSignLimitsV1 {
            profile_limits: ExactCommonLinearCycleProfileLimitsV1 {
                max_edges: fixture.profile.edge_ids().len() - 1,
                ..exact.profile_limits
            },
            ..exact
        },
        ExactCommonEffectiveGeneratorSignLimitsV1 {
            max_edges: exact.max_edges - 1,
            ..exact
        },
        ExactCommonEffectiveGeneratorSignLimitsV1 {
            max_faces: exact.max_faces - 1,
            ..exact
        },
        ExactCommonEffectiveGeneratorSignLimitsV1 {
            max_work: exact.max_work - 1,
            ..exact
        },
        ExactCommonEffectiveGeneratorSignLimitsV1 {
            max_retained_bytes: exact.max_retained_bytes - 1,
            ..exact
        },
        ExactCommonEffectiveGeneratorSignLimitsV1 {
            max_peak_bytes: exact.max_peak_bytes - 1,
            ..exact
        },
    ] {
        assert_eq!(
            prove_exact_common_effective_generator_sign_v1(
                &fixture.geometry,
                &fixture.audit,
                fixture.fixed_face,
                &fixture.schedule,
                &fixture.profile,
                one_short,
            ),
            Err(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)
        );
    }
}

#[test]
fn meter_fails_closed_on_checked_overflow() {
    assert_eq!(
        retained_bytes_v1(usize::MAX),
        Err(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)
    );
    assert_eq!(
        temporary_bytes_v1(usize::MAX, usize::MAX),
        Err(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)
    );
    let unbounded = ExactCommonEffectiveGeneratorSignLimitsV1 {
        profile_limits: ExactCommonLinearCycleProfileLimitsV1::default(),
        max_edges: usize::MAX,
        max_faces: usize::MAX,
        max_work: usize::MAX,
        max_retained_bytes: usize::MAX,
        max_peak_bytes: usize::MAX,
    };
    let mut work = MeterV1::new(unbounded);
    work.work = usize::MAX;
    assert_eq!(
        work.charge_work(1),
        Err(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)
    );
    let mut storage = MeterV1::new(unbounded);
    storage.temporary_bytes = usize::MAX;
    assert_eq!(
        storage.begin_temporary(1),
        Err(ExactCommonEffectiveGeneratorSignErrorV1::ResourceLimit)
    );
}
