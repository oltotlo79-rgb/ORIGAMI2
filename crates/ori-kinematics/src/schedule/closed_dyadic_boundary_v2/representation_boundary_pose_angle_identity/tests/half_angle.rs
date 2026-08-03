use super::*;

#[test]
fn representation_boundary_pose_half_angle_mirrors_public_point_bits_and_requires_exact_box_membership()
 {
    let fixture = half_angle_fixture_v2();
    let entry = &fixture.schedule.half_angle_entries[0];
    let lower = entry.u_domain[0].to_f64().unwrap();
    let upper = entry.u_domain[1].to_f64().unwrap();
    let affine_upper = lower + (upper - lower) * 1.0;
    assert_ne!(affine_upper.to_bits(), upper.to_bits());

    for upper_endpoint in [false, true] {
        let public = fixture
            .schedule
            .try_evaluate_v1(f64::from(upper_endpoint))
            .unwrap();
        let mut meter = resources::BoundaryWorkMeterV2::new(1_000_000);
        let mirrored = evaluate_pose::evaluate_half_angle_point_v2(
            entry,
            upper_endpoint,
            &mut meter,
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(
            mirrored.angle_degrees().to_bits(),
            public.as_slice()[0].angle_degrees().to_bits()
        );
        let enclosure = fixture
            .schedule
            .evaluate_endpoint_angle_box(upper_endpoint, fixture.schedule_limits)
            .unwrap()[0]
            .1;
        assert!(enclosure.lower() <= mirrored.angle_degrees());
        assert!(mirrored.angle_degrees() <= enclosure.upper());
    }

    let evidence =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            fixture.input_v2(),
        )
        .unwrap();
    evidence.revalidate_v2(fixture.input_v2()).unwrap();
    assert_eq!(
        evidence.closed_boundary_evidence_binding_fingerprint_v2(),
        fixture.closed_boundary.binding_fingerprint_v2()
    );
}

#[test]
fn representation_boundary_pose_half_angle_rejects_pose_bits_and_sealed_boundary_drift() {
    let fixture = half_angle_fixture_v2();
    let evidence =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            fixture.input_v2(),
        )
        .unwrap();

    let mut angles = fixture.upper_pose.hinge_angles().as_slice().to_vec();
    angles[0] = HingeAngle::new(
        angles[0].edge(),
        f64::from_bits(angles[0].angle_degrees().to_bits() + 1),
    )
    .unwrap();
    let one_ulp = CanonicalHingeAngles::new(angles).unwrap();
    let one_ulp_pose = fixture
        .geometry
        .solve_closed(
            &fixture.audit,
            fixture.upper_pose.fixed_face(),
            &one_ulp,
            FIXTURE_CLOSURE_TOLERANCE_V2,
        )
        .unwrap();
    let mut one_ulp_input = fixture.input_v2();
    one_ulp_input.upper_pose = &one_ulp_pose;
    assert_eq!(
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            one_ulp_input
        )
        .unwrap_err(),
        CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::BoundaryPoseMismatch
    );

    let mut drifted_schedule = fixture.schedule.clone();
    drifted_schedule.schedule_fingerprint_v2[0] ^= 0x80;
    let mut drifted_input = fixture.input_v2();
    drifted_input.schedule = &drifted_schedule;
    assert_eq!(
        evidence.revalidate_v2(drifted_input),
        Err(
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::CertificateBindingMismatch
        )
    );
}
