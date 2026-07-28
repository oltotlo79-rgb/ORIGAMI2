use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use super::*;
use crate::{
    CycleScheduleEntryInputV1, CycleScheduleLimitsV1, Point3, RationalCoefficientV1, TreeHinge,
    TreeKinematicsLimits,
};

pub(super) struct Fixture {
    pub(super) geometry: MaterialHingeGraphGeometry,
    pub(super) audit: MaterialHingeGraphAudit,
    pub(super) fixed_face: FaceId,
    pub(super) schedule: CanonicalCycleScheduleV1,
    pub(super) profile: ExactCommonLinearCycleProfileV1,
}

fn topology(faces: &[FaceId], edges: &[EdgeId]) -> TopologySnapshot {
    TopologySnapshot {
        source_revision: 1,
        faces: faces
            .iter()
            .copied()
            .map(|id| Face {
                id,
                key: FaceKey(id.canonical_bytes().repeat(2).try_into().unwrap()),
                outer: BoundaryWalk {
                    half_edges: Vec::new(),
                    signed_double_area: 1.0,
                },
                holes: Vec::new(),
                seams: Vec::new(),
                area: 0.5,
            })
            .collect(),
        edge_incidence: Vec::new(),
        hinge_adjacency: edges
            .iter()
            .map(|edge| FaceAdjacency {
                edge: *edge,
                first: faces[0],
                second: faces[1],
                assignment: FoldAssignment::Mountain,
            })
            .collect(),
        material_components: Vec::new(),
    }
}

fn schedule_entries(edges: &[EdgeId], slope: i64) -> Vec<CycleScheduleEntryInputV1> {
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
                    numerator: slope,
                    denominator: 1,
                },
            ],
        })
        .collect()
}

pub(super) fn bind_schedule_and_profile(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    edges: &[EdgeId],
    slope: i64,
) -> (CanonicalCycleScheduleV1, ExactCommonLinearCycleProfileV1) {
    let schedule = CanonicalCycleScheduleV1::prepare(
        geometry,
        audit,
        fixed_face,
        [0.0, 1.0],
        schedule_entries(edges, slope),
        CycleScheduleLimitsV1::default(),
    )
    .unwrap();
    let profile = schedule
        .prove_exact_common_linear_profile_v1(
            edges,
            ExactCommonLinearCycleProfileLimitsV1::default(),
        )
        .unwrap();
    (schedule, profile)
}

fn split_fixture(
    edge_count: usize,
    fixed_index: usize,
    storage_variants: bool,
    target_effective_sign: i8,
    offset_last: bool,
    mismatch_last: bool,
    slope: i64,
) -> Fixture {
    let namespace = ProjectId::schema_namespace([0x73; 16]);
    let faces = [b"split-face-a", b"split-face-b"].map(|name| FaceId::derive_v5(namespace, name));
    let edges = (0..edge_count)
        .map(|index| EdgeId::derive_v5(namespace, format!("split-edge-{index}").as_bytes()))
        .collect::<Vec<_>>();
    let fixed_face = faces[fixed_index];
    let moving_face = faces[1 - fixed_index];
    let mut hinges = Vec::new();
    for (index, edge) in edges.iter().copied().enumerate() {
        let reverse_faces = storage_variants && index % 2 == 1;
        let (left_face, right_face) = if reverse_faces {
            (moving_face, fixed_face)
        } else {
            (fixed_face, moving_face)
        };
        let fixed_side_sign = if left_face == fixed_face { 1_i8 } else { -1_i8 };
        let effective_sign = if mismatch_last && index + 1 == edge_count {
            -target_effective_sign
        } else {
            target_effective_sign
        };
        let line_generator_sign = fixed_side_sign * effective_sign;
        let assignment = if storage_variants && index % 2 == 1 {
            FoldAssignment::Valley
        } else {
            FoldAssignment::Mountain
        };
        let assignment_sign = match assignment {
            FoldAssignment::Mountain => 1_i8,
            FoldAssignment::Valley => -1_i8,
        };
        let axis_sign = line_generator_sign * assignment_sign;
        let base = 2.0 * index as f64;
        let (start_x, end_x) = if axis_sign == 1 {
            (base, base + 1.0)
        } else {
            (base + 1.0, base)
        };
        let y = if offset_last && index + 1 == edge_count {
            f64::from_bits(1)
        } else {
            0.0
        };
        hinges.push(TreeHinge::new_for_test(
            edge,
            assignment,
            left_face,
            right_face,
            Point3::new(start_x, y, 0.0).unwrap(),
            Point3::new(end_x, y, 0.0).unwrap(),
            Point3::new(f64::from(axis_sign), 0.0, 0.0).unwrap(),
        ));
    }
    hinges.sort_unstable_by_key(|hinge| hinge.edge().canonical_bytes());
    if storage_variants {
        hinges.rotate_left(1);
    }
    let mut canonical_faces = faces.to_vec();
    canonical_faces.sort_unstable_by_key(FaceId::canonical_bytes);
    let mut canonical_edges = edges;
    canonical_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let audit = MaterialHingeGraphAudit::prepare(
        &topology(&faces, &canonical_edges),
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    assert_eq!(audit.faces(), canonical_faces);
    assert_eq!(audit.spanning_hinges(), &canonical_edges[..1]);
    assert_eq!(audit.closure_hinges(), &canonical_edges[1..]);
    let geometry = MaterialHingeGraphGeometry::new_for_test(canonical_faces, hinges);
    let (schedule, profile) =
        bind_schedule_and_profile(&geometry, &audit, fixed_face, &canonical_edges, slope);
    Fixture {
        geometry,
        audit,
        fixed_face,
        schedule,
        profile,
    }
}

pub(super) fn prove(
    fixture: &Fixture,
) -> Result<
    ExactCommonSplitPairEffectiveGeneratorSignV1,
    ExactCommonSplitPairEffectiveGeneratorSignErrorV1,
> {
    prove_exact_common_split_pair_effective_generator_sign_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &fixture.schedule,
        &fixture.profile,
        ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
    )
}

#[test]
fn accepts_strict_two_and_three_hinge_split_pairs_and_exposes_no_authority() {
    for edge_count in [2, 3] {
        for fixed_index in [0, 1] {
            for storage_variants in [false, true] {
                let fixture = split_fixture(
                    edge_count,
                    fixed_index,
                    storage_variants,
                    1,
                    false,
                    false,
                    10,
                );
                let proof = prove(&fixture).unwrap();
                assert_eq!(
                    proof.model_id(),
                    EXACT_COMMON_SPLIT_PAIR_EFFECTIVE_GENERATOR_SIGN_MODEL_ID_V1
                );
                assert_eq!(proof.edge_ids(), fixture.profile.edge_ids());
                assert_eq!(proof.fixed_face(), fixture.fixed_face);
                let face_pair = proof.face_pair();
                assert!(face_pair[0].canonical_bytes() < face_pair[1].canonical_bytes());
                assert!(face_pair.contains(&fixture.fixed_face));
                assert_ne!(proof.moving_face(), fixture.fixed_face);
                assert_eq!(
                    proof.common_effective_sign(),
                    EffectiveGeneratorSignV1::Positive
                );
                assert!(!proof.authorizes_closure());
                assert!(!proof.authorizes_continuous_motion());
                assert!(!proof.authorizes_collision_clearance());
                assert!(!proof.authorizes_simulation_admission());
                assert!(!proof.authorizes_persistence());
                assert!(!proof.authorizes_apply());
                assert!(!proof.authorizes_project_mutation());
            }
        }
    }

    let negative = split_fixture(3, 0, true, -1, false, false, -10);
    assert_eq!(
        prove(&negative).unwrap().common_effective_sign(),
        EffectiveGeneratorSignV1::Negative
    );
}

#[test]
fn revalidation_requires_the_original_graph_instance() {
    let fixture = split_fixture(3, 0, true, 1, false, false, 10);
    let proof = prove(&fixture).unwrap();
    proof
        .revalidate_issuers_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.profile,
            ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
        )
        .unwrap();
    let shared_instance = fixture.geometry.clone();
    proof
        .revalidate_issuers_v1(
            &shared_instance,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.profile,
            ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
        )
        .unwrap();

    let equal_but_foreign = MaterialHingeGraphGeometry::new_for_test(
        fixture.geometry.face_ids().to_vec(),
        fixture.geometry.hinges().to_vec(),
    );
    assert_eq!(
        proof.revalidate_issuers_v1(
            &equal_but_foreign,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.profile,
            ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
        ),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::IssuerMismatch)
    );
}

#[test]
fn content_fingerprint_and_resource_accounting_are_deterministic() {
    let fixture = split_fixture(3, 0, false, 1, false, false, 10);
    let first = prove(&fixture).unwrap();
    let second = prove(&fixture).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.proof_fingerprint_v1, second.proof_fingerprint_v1);
    assert_eq!(
        first.proof_fingerprint_v1,
        [
            0x27, 0x10, 0x56, 0x11, 0xe1, 0x93, 0x99, 0x81, 0x35, 0x86, 0x80, 0x3b, 0xfb, 0xef,
            0x3d, 0x1b, 0x72, 0xe7, 0x10, 0x0d, 0xe1, 0xfe, 0xf3, 0xb6, 0xe6, 0xcf, 0x40, 0xeb,
            0xfb, 0x8d, 0xa2, 0xb6,
        ],
        "V1 proof fingerprint is a cross-runtime persistence contract",
    );

    let foreign_geometry = MaterialHingeGraphGeometry::new_for_test(
        fixture.geometry.face_ids().to_vec(),
        fixture.geometry.hinges().to_vec(),
    );
    let (foreign_schedule, foreign_profile) = bind_schedule_and_profile(
        &foreign_geometry,
        &fixture.audit,
        fixture.fixed_face,
        fixture.profile.edge_ids(),
        10,
    );
    let foreign = prove_exact_common_split_pair_effective_generator_sign_v1(
        &foreign_geometry,
        &fixture.audit,
        fixture.fixed_face,
        &foreign_schedule,
        &foreign_profile,
        ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
    )
    .unwrap();
    assert_ne!(first, foreign);
    assert_eq!(first.proof_fingerprint_v1, foreign.proof_fingerprint_v1);

    let variants = split_fixture(3, 0, true, 1, false, false, 10);
    let unbounded = ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
        profile_limits: ExactCommonLinearCycleProfileLimitsV1::default(),
        max_edges: usize::MAX,
        max_faces: usize::MAX,
        max_work: usize::MAX,
        max_retained_bytes: usize::MAX,
        max_peak_bytes: usize::MAX,
    };
    let mut canonical_meter = MeterV1::new(unbounded);
    prove_with_meter_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &fixture.schedule,
        &fixture.profile,
        &mut canonical_meter,
    )
    .unwrap();
    let mut variant_meter = MeterV1::new(unbounded);
    prove_with_meter_v1(
        &variants.geometry,
        &variants.audit,
        variants.fixed_face,
        &variants.schedule,
        &variants.profile,
        &mut variant_meter,
    )
    .unwrap();
    assert_eq!(canonical_meter.work, variant_meter.work);
    assert_eq!(canonical_meter.retained_bytes, variant_meter.retained_bytes);
    assert_eq!(canonical_meter.peak_bytes, variant_meter.peak_bytes);
}

#[test]
fn rejects_noncollinear_and_disagreeing_effective_generators() {
    let offset = split_fixture(3, 0, true, 1, true, false, 10);
    assert_eq!(
        prove(&offset),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::NonCollinearCarrier)
    );

    let mismatch = split_fixture(3, 0, true, 1, false, true, 10);
    assert_eq!(
        prove(&mismatch),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::EffectiveSignMismatch)
    );
}

#[test]
fn rejects_non_pair_geometry_and_invalid_audit_partition() {
    let fixture = split_fixture(3, 0, false, 1, false, false, 10);
    let mut invalid_hinges = fixture.geometry.hinges().to_vec();
    let original = invalid_hinges[2].clone();
    invalid_hinges[2] = TreeHinge::new_for_test(
        original.edge(),
        original.assignment(),
        original.left_face(),
        original.left_face(),
        original.start(),
        original.end(),
        original.axis(),
    );
    let invalid_geometry = MaterialHingeGraphGeometry::new_for_test(
        fixture.geometry.face_ids().to_vec(),
        invalid_hinges,
    );
    let (invalid_schedule, invalid_profile) = bind_schedule_and_profile(
        &invalid_geometry,
        &fixture.audit,
        fixture.fixed_face,
        fixture.profile.edge_ids(),
        10,
    );
    assert_eq!(
        prove_exact_common_split_pair_effective_generator_sign_v1(
            &invalid_geometry,
            &fixture.audit,
            fixture.fixed_face,
            &invalid_schedule,
            &invalid_profile,
            ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
        ),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::UnsupportedSplitPair)
    );

    let invalid_partition = MaterialHingeGraphAudit {
        faces: fixture.audit.faces().to_vec(),
        spanning_hinges: fixture.profile.edge_ids()[..2].to_vec(),
        closure_hinges: fixture.profile.edge_ids()[2..].to_vec(),
    };
    assert_eq!(
        prove_exact_common_split_pair_effective_generator_sign_v1(
            &fixture.geometry,
            &invalid_partition,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.profile,
            ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
        ),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::UnsupportedSplitPair)
    );

    let alternate_partition = MaterialHingeGraphAudit {
        faces: fixture.audit.faces().to_vec(),
        spanning_hinges: vec![fixture.profile.edge_ids()[1]],
        closure_hinges: vec![fixture.profile.edge_ids()[0], fixture.profile.edge_ids()[2]],
    };
    let (alternate_schedule, alternate_profile) = bind_schedule_and_profile(
        &fixture.geometry,
        &alternate_partition,
        fixture.fixed_face,
        fixture.profile.edge_ids(),
        10,
    );
    assert_eq!(
        prove_exact_common_split_pair_effective_generator_sign_v1(
            &fixture.geometry,
            &alternate_partition,
            fixture.fixed_face,
            &alternate_schedule,
            &alternate_profile,
            ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
        ),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::CarrierSetMismatch)
    );
}

#[test]
fn rejects_carrier_and_binding_tamper_without_weakening_tree_proof() {
    let fixture = split_fixture(3, 0, true, 1, false, false, 10);
    let foreign_edge = EdgeId::derive_v5(
        ProjectId::schema_namespace([0x74; 16]),
        b"foreign-audit-edge",
    );
    let foreign_audit = MaterialHingeGraphAudit {
        faces: fixture.audit.faces().to_vec(),
        spanning_hinges: vec![foreign_edge],
        closure_hinges: fixture.profile.edge_ids()[1..].to_vec(),
    };
    let (foreign_schedule, foreign_profile) = bind_schedule_and_profile(
        &fixture.geometry,
        &foreign_audit,
        fixture.fixed_face,
        fixture.profile.edge_ids(),
        10,
    );
    assert_eq!(
        prove_exact_common_split_pair_effective_generator_sign_v1(
            &fixture.geometry,
            &foreign_audit,
            fixture.fixed_face,
            &foreign_schedule,
            &foreign_profile,
            ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
        ),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::CarrierSetMismatch)
    );

    let other_fixed = fixture
        .audit
        .faces()
        .iter()
        .copied()
        .find(|face| *face != fixture.fixed_face)
        .unwrap();
    assert_eq!(
        prove_exact_common_split_pair_effective_generator_sign_v1(
            &fixture.geometry,
            &fixture.audit,
            other_fixed,
            &fixture.schedule,
            &fixture.profile,
            ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
        ),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::GraphBindingMismatch)
    );

    let changed_profile = split_fixture(3, 0, true, 1, false, false, 11);
    assert_eq!(
        prove_exact_common_split_pair_effective_generator_sign_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &changed_profile.profile,
            ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
        ),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ProfileIssuerMismatch)
    );

    assert_eq!(
        crate::prove_exact_common_effective_generator_sign_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.profile,
            crate::ExactCommonEffectiveGeneratorSignLimitsV1::default(),
        ),
        Err(crate::ExactCommonEffectiveGeneratorSignErrorV1::UnsupportedRootedCarrier)
    );
}

#[test]
fn proof_payload_tamper_fails_same_issuer_revalidation() {
    let fixture = split_fixture(3, 0, true, 1, false, false, 10);
    let proof = prove(&fixture).unwrap();
    let mut forged_hash = proof.clone();
    forged_hash.proof_fingerprint_v1[0] ^= 1;
    assert_eq!(
        forged_hash.revalidate_issuers_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.profile,
            ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
        ),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::IssuerMismatch)
    );

    let mut forged_edges = proof;
    forged_edges.canonical_edges.swap(0, 1);
    assert_eq!(
        forged_edges.revalidate_issuers_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.profile,
            ExactCommonSplitPairEffectiveGeneratorSignLimitsV1::default(),
        ),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::IssuerMismatch)
    );
}

#[test]
fn limits_succeed_at_equality_and_fail_one_short() {
    let fixture = split_fixture(3, 0, true, 1, false, false, 10);
    let unbounded = ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
        profile_limits: ExactCommonLinearCycleProfileLimitsV1::default(),
        max_edges: MAX_EDGES_V1,
        max_faces: REQUIRED_FACES_V1,
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
    let exact = ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
        profile_limits: ExactCommonLinearCycleProfileLimitsV1::default(),
        max_edges: fixture.profile.edge_ids().len(),
        max_faces: fixture.geometry.face_ids().len(),
        max_work: audit_meter.work,
        max_retained_bytes: audit_meter.retained_bytes,
        max_peak_bytes: audit_meter.peak_bytes,
    };
    let proof = prove_exact_common_split_pair_effective_generator_sign_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &fixture.schedule,
        &fixture.profile,
        exact,
    )
    .unwrap();

    for one_short in [
        ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
            profile_limits: ExactCommonLinearCycleProfileLimitsV1 {
                max_edges: fixture.profile.edge_ids().len() - 1,
                ..exact.profile_limits
            },
            ..exact
        },
        ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
            max_edges: exact.max_edges - 1,
            ..exact
        },
        ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
            max_faces: exact.max_faces - 1,
            ..exact
        },
        ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
            max_work: exact.max_work - 1,
            ..exact
        },
        ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
            max_retained_bytes: exact.max_retained_bytes - 1,
            ..exact
        },
        ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
            max_peak_bytes: exact.max_peak_bytes - 1,
            ..exact
        },
    ] {
        assert_eq!(
            prove_exact_common_split_pair_effective_generator_sign_v1(
                &fixture.geometry,
                &fixture.audit,
                fixture.fixed_face,
                &fixture.schedule,
                &fixture.profile,
                one_short,
            ),
            Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)
        );
    }

    let comparison_work = retained_bytes_v1(fixture.profile.edge_ids().len())
        .unwrap()
        .checked_add(1)
        .unwrap();
    let revalidation_work = audit_meter
        .work
        .checked_add(1)
        .and_then(|work| work.checked_add(comparison_work))
        .unwrap();
    let exact_revalidation = ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
        max_work: revalidation_work,
        ..exact
    };
    proof
        .revalidate_issuers_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.profile,
            exact_revalidation,
        )
        .unwrap();
    assert_eq!(
        proof.revalidate_issuers_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.profile,
            ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
                max_work: revalidation_work - 1,
                ..exact_revalidation
            },
        ),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)
    );
}

#[test]
fn meter_fails_closed_on_checked_overflow() {
    assert_eq!(
        retained_bytes_v1(usize::MAX),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)
    );
    assert_eq!(
        temporary_bytes_v1(usize::MAX, usize::MAX),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)
    );
    let unbounded = ExactCommonSplitPairEffectiveGeneratorSignLimitsV1 {
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
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)
    );
    let mut storage = MeterV1::new(unbounded);
    storage.temporary_bytes = usize::MAX;
    assert_eq!(
        storage.begin_temporary(1),
        Err(ExactCommonSplitPairEffectiveGeneratorSignErrorV1::ResourceLimit)
    );
}
