use ori_kinematics::{
    CanonicalCycleScheduleV1, CycleScheduleEntryInputV1, CycleScheduleLimitsV1,
    ExactCommonLinearCycleProfileV1, ExactCommonSplitPairEffectiveGeneratorSignV1,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry, RationalCoefficientV1,
    prove_exact_common_split_pair_effective_generator_sign_v1,
};

use super::{
    MultiHingeReliefUnionCertificateV2, MultiHingeReliefUnionGapReportV2,
    MultiHingeReliefUnionLimitsV2, SplitHingeUnionExteriorReliefAssumptionErrorV1,
    SplitHingeUnionExteriorReliefAssumptionLimitsV1, SplitHingeUnionExteriorReliefAssumptionV1,
    certify_multi_hinge_relief_union_v2, diagnose_multi_hinge_relief_union_gaps_v2,
    prove_split_hinge_union_exterior_relief_assumption_v1,
    revalidate_split_hinge_union_exterior_relief_assumption_v1, tests::segmented_crease,
};
use crate::{
    HingeReliefLinearAngleScheduleV1, HingeReliefPolicyLimitsV1, HingeReliefPolicyRecordV1,
    NativeHingeReliefLocalIntervalCertificateV1, NativeHingeReliefPrerequisiteV1,
    certify_hinge_relief_local_intervals_v1, prepare_hinge_relief_prerequisite_v1,
};

const THICKNESS_MM: f64 = 0.1;

struct PhaseBFixture {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    fixed_face: ori_domain::FaceId,
    schedule: CanonicalCycleScheduleV1,
    common_profile: ExactCommonLinearCycleProfileV1,
    split_sign: ExactCommonSplitPairEffectiveGeneratorSignV1,
    gaps: MultiHingeReliefUnionGapReportV2,
    union: MultiHingeReliefUnionCertificateV2,
    policies: Vec<HingeReliefPolicyRecordV1>,
    local_schedules: Vec<HingeReliefLinearAngleScheduleV1>,
    prerequisite: NativeHingeReliefPrerequisiteV1,
    local: NativeHingeReliefLocalIntervalCertificateV1,
    policy_limits: HingeReliefPolicyLimitsV1,
    union_limits: MultiHingeReliefUnionLimitsV2,
    limits: SplitHingeUnionExteriorReliefAssumptionLimitsV1,
}

fn common_schedule(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: ori_domain::FaceId,
    initial_angle_degrees: f64,
    linear_coefficient: i64,
) -> CanonicalCycleScheduleV1 {
    let mut edges = geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(ori_domain::EdgeId::canonical_bytes);
    let entries = edges
        .into_iter()
        .map(|edge| CycleScheduleEntryInputV1 {
            edge,
            initial_angle_degrees_bits: initial_angle_degrees.to_bits(),
            chebyshev_coefficients: vec![
                RationalCoefficientV1 {
                    numerator: 0,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: linear_coefficient,
                    denominator: 1,
                },
            ],
        })
        .collect();
    CanonicalCycleScheduleV1::prepare(
        geometry,
        audit,
        fixed_face,
        [0.0, 1.0],
        entries,
        CycleScheduleLimitsV1::default(),
    )
    .expect("prepare one common non-constant ordinary schedule")
}

fn phase_b_fixture(
    edge_count: usize,
    initial_angle_degrees: f64,
    linear_coefficient: i64,
    radial_depth_mm: f64,
    bevel_angle_degrees: f64,
) -> PhaseBFixture {
    let (geometry, audit, _, fixed_face) = segmented_crease(edge_count, 1);
    let schedule = common_schedule(
        &geometry,
        &audit,
        fixed_face,
        initial_angle_degrees,
        linear_coefficient,
    );
    let limits = SplitHingeUnionExteriorReliefAssumptionLimitsV1::default();
    let mut edges = geometry
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(ori_domain::EdgeId::canonical_bytes);
    let common_profile = schedule
        .prove_exact_common_linear_profile_v1(&edges, limits.profile_limits)
        .expect("prove the exact common linear carrier");
    let split_sign = prove_exact_common_split_pair_effective_generator_sign_v1(
        &geometry,
        &audit,
        fixed_face,
        &schedule,
        &common_profile,
        limits.split_sign_limits,
    )
    .expect("prove one logical split-pair effective sign");

    let union_limits = MultiHingeReliefUnionLimitsV2::default();
    let gaps = diagnose_multi_hinge_relief_union_gaps_v2(
        &geometry,
        &audit,
        fixed_face,
        &schedule,
        THICKNESS_MM,
        union_limits,
    )
    .expect("diagnose the one split-pair relief gap");
    let mut gap_hinges = gaps
        .gaps()
        .iter()
        .flat_map(|gap| gap.hinges())
        .copied()
        .collect::<Vec<_>>();
    gap_hinges.sort_unstable_by_key(|hinge| hinge.hinge().canonical_bytes());
    let policies = gap_hinges
        .iter()
        .map(|hinge| HingeReliefPolicyRecordV1 {
            edge: hinge.hinge(),
            cutout_width_mm: radial_depth_mm,
            bevel_angle_degrees,
            material_thickness_mm: THICKNESS_MM,
        })
        .collect::<Vec<_>>();
    let local_schedules = gap_hinges
        .iter()
        .map(|hinge| HingeReliefLinearAngleScheduleV1 {
            edge: hinge.hinge(),
            source_angle_degrees: f64::from_bits(hinge.source_angle_bits()),
            target_angle_degrees: f64::from_bits(hinge.target_angle_bits()),
        })
        .collect::<Vec<_>>();
    let policy_limits = HingeReliefPolicyLimitsV1::default();
    let prerequisite =
        prepare_hinge_relief_prerequisite_v1(&geometry, THICKNESS_MM, &policies, policy_limits)
            .expect("prepare exact hinge-relief policy");
    let local = certify_hinge_relief_local_intervals_v1(
        &prerequisite,
        &geometry,
        THICKNESS_MM,
        &policies,
        &local_schedules,
        policy_limits,
    )
    .expect("certify exact local relief intervals");
    let union = certify_multi_hinge_relief_union_v2(
        &gaps,
        &geometry,
        &audit,
        fixed_face,
        &schedule,
        THICKNESS_MM,
        &prerequisite,
        &local,
        &policies,
        &local_schedules,
        policy_limits,
        union_limits,
    )
    .expect("certify the complete split-hinge local union");

    PhaseBFixture {
        geometry,
        audit,
        fixed_face,
        schedule,
        common_profile,
        split_sign,
        gaps,
        union,
        policies,
        local_schedules,
        prerequisite,
        local,
        policy_limits,
        union_limits,
        limits,
    }
}

fn positive_fixture(edge_count: usize) -> PhaseBFixture {
    // The ordinary profile is 60 + 5x, x in [-1, 1]. The outward Clenshaw
    // root box is deliberately wider (approximately [45, 75]) but remains
    // strictly inside the supported (0, 90] Phase-B angle domain.
    phase_b_fixture(edge_count, 60.0, 5, 7.0, 1.0)
}

fn prove_with_limits(
    fixture: &PhaseBFixture,
    limits: SplitHingeUnionExteriorReliefAssumptionLimitsV1,
) -> Result<SplitHingeUnionExteriorReliefAssumptionV1, SplitHingeUnionExteriorReliefAssumptionErrorV1>
{
    prove_with_all_limits(fixture, fixture.policy_limits, fixture.union_limits, limits)
}

fn prove_with_all_limits(
    fixture: &PhaseBFixture,
    policy_limits: HingeReliefPolicyLimitsV1,
    union_limits: MultiHingeReliefUnionLimitsV2,
    limits: SplitHingeUnionExteriorReliefAssumptionLimitsV1,
) -> Result<SplitHingeUnionExteriorReliefAssumptionV1, SplitHingeUnionExteriorReliefAssumptionErrorV1>
{
    prove_split_hinge_union_exterior_relief_assumption_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &fixture.schedule,
        &fixture.common_profile,
        &fixture.split_sign,
        &fixture.gaps,
        &fixture.union,
        THICKNESS_MM,
        &fixture.prerequisite,
        &fixture.local,
        &fixture.policies,
        &fixture.local_schedules,
        policy_limits,
        union_limits,
        limits,
    )
}

fn prove(
    fixture: &PhaseBFixture,
) -> Result<SplitHingeUnionExteriorReliefAssumptionV1, SplitHingeUnionExteriorReliefAssumptionErrorV1>
{
    prove_with_limits(fixture, fixture.limits)
}

fn revalidate(
    evidence: &SplitHingeUnionExteriorReliefAssumptionV1,
    fixture: &PhaseBFixture,
    geometry: &MaterialHingeGraphGeometry,
) -> Result<(), SplitHingeUnionExteriorReliefAssumptionErrorV1> {
    revalidate_split_hinge_union_exterior_relief_assumption_v1(
        evidence,
        geometry,
        &fixture.audit,
        fixture.fixed_face,
        &fixture.schedule,
        &fixture.common_profile,
        &fixture.split_sign,
        &fixture.gaps,
        &fixture.union,
        THICKNESS_MM,
        &fixture.prerequisite,
        &fixture.local,
        &fixture.policies,
        &fixture.local_schedules,
        fixture.policy_limits,
        fixture.union_limits,
        fixture.limits,
    )
}

#[test]
fn production_two_and_three_segment_split_pairs_prove_phase_b_only() {
    let mut hashes = Vec::new();
    for edge_count in [2_usize, 3] {
        let fixture = positive_fixture(edge_count);
        let root_boxes = fixture
            .schedule
            .evaluate_angle_box_dyadic(0, 0, fixture.limits.schedule_limits)
            .expect("the positive root box is physical");
        assert!(root_boxes.iter().all(|(_, interval)| {
            interval.lower() > 0.0
                && interval.upper() <= 90.0
                && interval.lower().to_bits() == root_boxes[0].1.lower().to_bits()
                && interval.upper().to_bits() == root_boxes[0].1.upper().to_bits()
        }));
        let evidence = prove(&fixture).expect("the bounded Phase-B assumption must be recognized");
        assert_eq!(
            evidence.model_id(),
            super::SPLIT_HINGE_UNION_EXTERIOR_RELIEF_ASSUMPTION_MODEL_ID_V1
        );
        assert_eq!(evidence.face_pair(), fixture.split_sign.face_pair());
        assert_eq!(evidence.fixed_face(), fixture.fixed_face);
        assert_eq!(evidence.moving_face(), fixture.split_sign.moving_face());
        assert_eq!(evidence.edge_ids(), fixture.split_sign.edge_ids());
        assert_eq!(
            evidence.common_effective_sign(),
            fixture.split_sign.common_effective_sign()
        );
        assert!(evidence.recognizes_union_exterior_relief_assumption());
        assert!(evidence.work_used() > 0);
        assert!(evidence.maximum_exact_bits() > 0);
        assert!(evidence.total_exact_bits() > 0);
        assert!(evidence.retained_storage_bytes() > 0);
        assert!(evidence.peak_storage_bytes() >= evidence.retained_storage_bytes());

        assert!(!evidence.authorizes_union_exterior_clearance());
        assert!(!evidence.authorizes_whole_path());
        assert!(!evidence.authorizes_continuous_motion());
        assert!(!evidence.authorizes_collision_clearance());
        assert!(!evidence.authorizes_collision_free_classification());
        assert!(!evidence.authorizes_shared_hinge_admission());
        assert!(!evidence.authorizes_simulation_admission());
        assert!(!evidence.authorizes_persistence());
        assert!(!evidence.authorizes_apply());
        assert!(!evidence.authorizes_project_mutation());

        let same_instance_clone = fixture.geometry.clone();
        revalidate(&evidence, &fixture, &same_instance_clone)
            .expect("a geometry clone preserves the exact issuer identity");
        hashes.push(evidence.content_hash_v1());
    }
    assert_ne!(
        hashes[0], hashes[1],
        "two- and three-segment carriers are distinct evidence"
    );
}

#[test]
fn structurally_equal_detached_geometry_is_not_the_same_issuer() {
    let original = positive_fixture(3);
    let evidence = prove(&original).unwrap();
    let detached = positive_fixture(3);
    assert_eq!(original.geometry, detached.geometry);
    assert!(!original.geometry.same_instance(&detached.geometry));
    let detached_evidence = prove(&detached).unwrap();
    assert_eq!(
        evidence.content_hash_v1(),
        detached_evidence.content_hash_v1(),
        "content addressing is structural while issuer identity stays runtime-local"
    );
    assert!(matches!(
        revalidate(&evidence, &detached, &detached.geometry),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::IssuerMismatch)
    ));
}

#[test]
fn root_angle_lower_and_upper_domain_fail_closed() {
    // Both exact endpoint schedules remain physical and pass the upstream
    // local-relief proof. Phase B rejects their wider outward root boxes.
    let nonpositive_lower = phase_b_fixture(3, 10.0, 5, 7.0, 1.0);
    assert!(
        nonpositive_lower
            .schedule
            .evaluate_angle_box_dyadic(0, 0, nonpositive_lower.limits.schedule_limits)
            .is_err(),
        "the ordinary outward evaluator rejects its nonpositive lower enclosure"
    );
    assert!(matches!(
        prove(&nonpositive_lower),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::AngleDomain)
    ));

    let above_ninety_upper = phase_b_fixture(3, 80.0, 5, 7.0, 1.0);
    let upper_boxes = above_ninety_upper
        .schedule
        .evaluate_angle_box_dyadic(0, 0, above_ninety_upper.limits.schedule_limits)
        .unwrap();
    assert!(
        upper_boxes
            .iter()
            .any(|(_, interval)| interval.upper() > 90.0)
    );
    assert!(matches!(
        prove(&above_ninety_upper),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::AngleDomain)
    ));
}

#[test]
fn exact_relief_and_finite_corridor_reject_independently() {
    // Endpoint-local relief passes: .12*55 >= 60*.1. The conservative root
    // lower box is about 45 degrees, so the stronger Phase-B inequality fails.
    let insufficient_root_relief = phase_b_fixture(3, 60.0, 5, 0.12, 60.0);
    assert!(matches!(
        prove(&insufficient_root_relief),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ReliefInequality)
    ));

    // The far boundary is exactly 5 mm from the carrier. The closed radial
    // envelope admits equality before the one-sided negative below.
    let closed_radial_boundary = phase_b_fixture(3, 60.0, 5, 5.0, 2.0);
    prove(&closed_radial_boundary).expect("the exact radial boundary is closed");

    // The rectangle extends exactly 5 mm from its split carrier. A 4.9 mm
    // policy clears all upstream relief checks but not the exact face envelope.
    let narrow_corridor = phase_b_fixture(3, 60.0, 5, 4.9, 2.0);
    assert!(matches!(
        prove(&narrow_corridor),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::CorridorEnvelope)
    ));
}

#[test]
fn exact_resource_observations_accept_equality_and_reject_one_short() {
    let fixture = positive_fixture(3);
    let evidence = prove(&fixture).unwrap();

    let mut exact = fixture.limits;
    exact.max_work = evidence.work_used();
    exact.max_retained_bytes = evidence.retained_storage_bytes();
    exact.max_peak_bytes = evidence.peak_storage_bytes();
    exact.max_exact_bits_per_rational = evidence.maximum_exact_bits();
    exact.max_total_exact_bits = evidence.total_exact_bits();
    let equality = prove_with_limits(&fixture, exact)
        .expect("all four observed Phase-B resource limits admit equality");
    assert_eq!(equality.work_used(), evidence.work_used());
    assert_eq!(
        equality.retained_storage_bytes(),
        evidence.retained_storage_bytes()
    );
    assert_eq!(equality.peak_storage_bytes(), evidence.peak_storage_bytes());
    assert_eq!(equality.maximum_exact_bits(), evidence.maximum_exact_bits());
    assert_eq!(equality.total_exact_bits(), evidence.total_exact_bits());

    let mut one_short = fixture.limits;
    one_short.max_edges = 2;
    assert!(matches!(
        prove_with_limits(&fixture, one_short),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ResourceLimit)
    ));

    let mut one_short = fixture.limits;
    one_short.max_work = evidence.work_used() - 1;
    assert!(matches!(
        prove_with_limits(&fixture, one_short),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ResourceLimit)
    ));

    let mut one_short = fixture.limits;
    one_short.max_retained_bytes = evidence.retained_storage_bytes() - 1;
    assert!(matches!(
        prove_with_limits(&fixture, one_short),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ResourceLimit)
    ));

    let mut one_short = fixture.limits;
    one_short.max_peak_bytes = evidence.peak_storage_bytes() - 1;
    assert!(matches!(
        prove_with_limits(&fixture, one_short),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ResourceLimit)
    ));

    let mut one_short = fixture.limits;
    one_short.max_exact_bits_per_rational = evidence.maximum_exact_bits() - 1;
    assert!(matches!(
        prove_with_limits(&fixture, one_short),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ResourceLimit)
    ));

    let mut one_short = fixture.limits;
    one_short.max_total_exact_bits = evidence.total_exact_bits() - 1;
    assert!(matches!(
        prove_with_limits(&fixture, one_short),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ResourceLimit)
    ));

    let boundary_lengths = fixture
        .geometry
        .face_ids()
        .iter()
        .map(|face| {
            fixture
                .geometry
                .face_boundary_vertices(*face)
                .unwrap()
                .len()
        })
        .collect::<Vec<_>>();
    let mut one_short = fixture.limits;
    one_short.max_boundary_vertices_per_face = boundary_lengths.iter().copied().max().unwrap() - 1;
    assert!(matches!(
        prove_with_limits(&fixture, one_short),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ResourceLimit)
    ));

    let mut one_short = fixture.limits;
    one_short.max_total_boundary_vertices = boundary_lengths.iter().sum::<usize>() - 1;
    assert!(matches!(
        prove_with_limits(&fixture, one_short),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ResourceLimit)
    ));
}

#[test]
fn stale_upstream_payloads_and_relief_inputs_fail_closed() {
    let fixture = positive_fixture(3);

    let mut stale_gaps = fixture.gaps.clone();
    stale_gaps.content_hash[0] ^= 1;
    assert!(matches!(
        prove_split_hinge_union_exterior_relief_assumption_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.common_profile,
            &fixture.split_sign,
            &stale_gaps,
            &fixture.union,
            THICKNESS_MM,
            &fixture.prerequisite,
            &fixture.local,
            &fixture.policies,
            &fixture.local_schedules,
            fixture.policy_limits,
            fixture.union_limits,
            fixture.limits,
        ),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignUnion)
    ));

    let mut stale_union = fixture.union.clone();
    stale_union.content_hash[0] ^= 1;
    assert!(matches!(
        prove_split_hinge_union_exterior_relief_assumption_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.common_profile,
            &fixture.split_sign,
            &fixture.gaps,
            &stale_union,
            THICKNESS_MM,
            &fixture.prerequisite,
            &fixture.local,
            &fixture.policies,
            &fixture.local_schedules,
            fixture.policy_limits,
            fixture.union_limits,
            fixture.limits,
        ),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignUnion)
    ));

    let mut stale_policies = fixture.policies.clone();
    stale_policies[0].cutout_width_mm += 0.25;
    assert!(matches!(
        prove_split_hinge_union_exterior_relief_assumption_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.common_profile,
            &fixture.split_sign,
            &fixture.gaps,
            &fixture.union,
            THICKNESS_MM,
            &fixture.prerequisite,
            &fixture.local,
            &stale_policies,
            &fixture.local_schedules,
            fixture.policy_limits,
            fixture.union_limits,
            fixture.limits,
        ),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignRelief)
    ));

    let mut stale_schedules = fixture.local_schedules.clone();
    stale_schedules[0].source_angle_degrees =
        f64::from_bits(stale_schedules[0].source_angle_degrees.to_bits() + 1);
    assert!(matches!(
        prove_split_hinge_union_exterior_relief_assumption_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.common_profile,
            &fixture.split_sign,
            &fixture.gaps,
            &fixture.union,
            THICKNESS_MM,
            &fixture.prerequisite,
            &fixture.local,
            &fixture.policies,
            &stale_schedules,
            fixture.policy_limits,
            fixture.union_limits,
            fixture.limits,
        ),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignRelief)
    ));
}

#[test]
fn every_nested_resource_envelope_is_freshly_enforced() {
    let fixture = positive_fixture(3);

    let mut profile_short = fixture.limits;
    profile_short.profile_limits.max_work = 1;
    profile_short.split_sign_limits.profile_limits = profile_short.profile_limits;
    assert!(matches!(
        prove_with_limits(&fixture, profile_short),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignCommonProfile)
    ));

    let mut sign_short = fixture.limits;
    sign_short.split_sign_limits.max_work = 1;
    assert!(matches!(
        prove_with_limits(&fixture, sign_short),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignSplitPairSign)
    ));

    let mut schedule_short = fixture.limits;
    schedule_short.schedule_limits.max_work = 1;
    assert!(matches!(
        prove_with_limits(&fixture, schedule_short),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::AngleDomain)
    ));

    let policy_short = HingeReliefPolicyLimitsV1 { max_records: 2 };
    assert!(matches!(
        prove_with_all_limits(&fixture, policy_short, fixture.union_limits, fixture.limits,),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignRelief)
    ));

    let mut union_short = fixture.union_limits;
    union_short.max_work = 1;
    assert!(matches!(
        prove_with_all_limits(&fixture, fixture.policy_limits, union_short, fixture.limits,),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignUnion)
    ));
}

#[test]
fn foreign_common_profile_split_sign_and_invalid_limits_fail_closed() {
    let fixture = positive_fixture(3);
    let different_profile = phase_b_fixture(3, 60.0, 6, 7.0, 1.0);
    assert!(matches!(
        prove_split_hinge_union_exterior_relief_assumption_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &different_profile.common_profile,
            &fixture.split_sign,
            &fixture.gaps,
            &fixture.union,
            THICKNESS_MM,
            &fixture.prerequisite,
            &fixture.local,
            &fixture.policies,
            &fixture.local_schedules,
            fixture.policy_limits,
            fixture.union_limits,
            fixture.limits,
        ),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignCommonProfile)
    ));

    let detached = positive_fixture(3);
    assert!(matches!(
        prove_split_hinge_union_exterior_relief_assumption_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            &fixture.common_profile,
            &detached.split_sign,
            &fixture.gaps,
            &fixture.union,
            THICKNESS_MM,
            &fixture.prerequisite,
            &fixture.local,
            &fixture.policies,
            &fixture.local_schedules,
            fixture.policy_limits,
            fixture.union_limits,
            fixture.limits,
        ),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::ForeignSplitPairSign)
    ));

    let mut invalid = fixture.limits;
    invalid.max_edges = 1;
    assert!(matches!(
        prove_with_limits(&fixture, invalid),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::InvalidLimits)
    ));

    let mut invalid = fixture.limits;
    invalid.split_sign_limits.profile_limits.max_work -= 1;
    assert!(matches!(
        prove_with_limits(&fixture, invalid),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::InvalidLimits)
    ));

    let mut invalid = fixture.limits;
    invalid.max_work = usize::MAX;
    assert!(matches!(
        prove_with_limits(&fixture, invalid),
        Err(SplitHingeUnionExteriorReliefAssumptionErrorV1::InvalidLimits)
    ));
}
