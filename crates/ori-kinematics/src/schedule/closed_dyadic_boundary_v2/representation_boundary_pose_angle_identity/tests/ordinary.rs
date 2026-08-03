use super::*;

#[test]
fn representation_boundary_pose_ordinary_uses_literal_normalized_endpoints_and_exact_instances() {
    let fixture = ordinary_fixture_v2();
    assert!(fixture.schedule.try_evaluate_v1(0.0).is_err());
    assert!(fixture.schedule.try_evaluate_v1(1.0).is_err());
    assert_eq!(
        fixture.lower_pose.hinge_angles().as_slice()[0]
            .angle_degrees()
            .to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(
        fixture.upper_pose.hinge_angles().as_slice()[0]
            .angle_degrees()
            .to_bits(),
        45.0_f64.to_bits()
    );

    let evidence =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            fixture.input_v2(),
        )
        .unwrap();
    assert_eq!(
        evidence.model_id_v2(),
        CANONICAL_CYCLE_SCHEDULE_REPRESENTATION_BOUNDARY_POSE_ANGLE_IDENTITY_EVIDENCE_MODEL_ID_V2
    );
    assert_eq!(
        evidence.representation_boundary_pose_angle_identity_count_v2(),
        2
    );
    assert_eq!(evidence.hinge_count_v2(), 1);
    assert_eq!(evidence.fixed_face_v2(), fixture.lower_pose.fixed_face());
    assert!(evidence.matches_geometry_instance_v2(&fixture.geometry));
    assert!(evidence.matches_pose_instances_v2(&fixture.lower_pose, &fixture.upper_pose));
    assert!(
        evidence.matches_representation_boundary_pose_angle_identity_instance_v2(
            false,
            &fixture.lower_pose
        )
    );
    assert!(
        evidence.matches_representation_boundary_pose_angle_identity_instance_v2(
            true,
            &fixture.upper_pose
        )
    );
    assert!(
        !evidence.matches_representation_boundary_pose_angle_identity_instance_v2(
            false,
            &fixture.upper_pose
        )
    );
    assert!(!evidence.authorizes_source_target_identity());
    assert!(!evidence.authorizes_current_requested_identity());
    assert!(!evidence.authorizes_application_parameter_identity());
    assert!(!evidence.authorizes_direction());
    assert!(!evidence.authorizes_layer_order());
    assert!(!evidence.authorizes_exact_closure());
    assert!(!evidence.authorizes_transform_realization());
    assert!(!evidence.authorizes_pose_realization());
    assert!(!evidence.authorizes_continuous_motion());
    assert!(!evidence.authorizes_collision_clearance());
    assert!(!evidence.authorizes_layer_transport());
    assert!(!evidence.authorizes_project_mutation());
    assert!(!evidence.authorizes_apply());
    assert!(!evidence.authorizes_viewer());
    assert!(!evidence.authorizes_export());
    evidence.revalidate_v2(fixture.input_v2()).unwrap();

    let lower_clone = fixture.lower_pose.clone();
    let upper_clone = fixture.upper_pose.clone();
    let mut cloned_input = fixture.input_v2();
    cloned_input.lower_pose = &lower_clone;
    cloned_input.upper_pose = &upper_clone;
    evidence.revalidate_v2(cloned_input).unwrap();

    let fresh_lower = fixture.fresh_lower_pose_v2();
    let mut fresh_input = fixture.input_v2();
    fresh_input.lower_pose = &fresh_lower;
    let fresh_evidence =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            fresh_input,
        )
        .unwrap();
    assert_eq!(
        evidence.binding_fingerprint_v2(),
        fresh_evidence.binding_fingerprint_v2()
    );
    assert!(!evidence.matches_pose_instances_v2(&fresh_lower, &fixture.upper_pose));
    assert_eq!(
        evidence.revalidate_v2(fresh_input),
        Err(
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::CertificateBindingMismatch
        )
    );
}

#[test]
fn representation_boundary_pose_ordinary_rejects_swaps_bits_fixed_face_and_geometry_aba() {
    let fixture = ordinary_fixture_v2();
    let evidence =
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            fixture.input_v2(),
        )
        .unwrap();

    let mut swapped = fixture.input_v2();
    swapped.lower_pose = &fixture.upper_pose;
    swapped.upper_pose = &fixture.lower_pose;
    assert_eq!(
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            swapped
        )
        .unwrap_err(),
        CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::BoundaryPoseMismatch
    );

    let mut angles = fixture.lower_pose.hinge_angles().as_slice().to_vec();
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
            fixture.lower_pose.fixed_face(),
            &one_ulp,
            FIXTURE_CLOSURE_TOLERANCE_V2,
        )
        .unwrap();
    let mut one_ulp_input = fixture.input_v2();
    one_ulp_input.lower_pose = &one_ulp_pose;
    assert_eq!(
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            one_ulp_input
        )
        .unwrap_err(),
        CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::BoundaryPoseMismatch
    );

    let alternate_fixed = fixture
        .geometry
        .face_ids()
        .iter()
        .copied()
        .find(|face| *face != fixture.lower_pose.fixed_face())
        .unwrap();
    let alternate_lower = fixture
        .geometry
        .solve_closed(
            &fixture.audit,
            alternate_fixed,
            fixture.lower_pose.hinge_angles(),
            FIXTURE_CLOSURE_TOLERANCE_V2,
        )
        .unwrap();
    let alternate_upper = fixture
        .geometry
        .solve_closed(
            &fixture.audit,
            alternate_fixed,
            fixture.upper_pose.hinge_angles(),
            FIXTURE_CLOSURE_TOLERANCE_V2,
        )
        .unwrap();
    let mut alternate_input = fixture.input_v2();
    alternate_input.lower_pose = &alternate_lower;
    alternate_input.upper_pose = &alternate_upper;
    assert_eq!(
        prove_canonical_cycle_schedule_representation_boundary_pose_angle_identity_evidence_v2(
            alternate_input
        )
        .unwrap_err(),
        CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::ScheduleBindingMismatch
    );

    let foreign_geometry = MaterialHingeGraphGeometry::new_for_test(
        fixture.geometry.face_ids().to_vec(),
        fixture.geometry.hinges().to_vec(),
    );
    let foreign_lower = foreign_geometry
        .solve_closed(
            &fixture.audit,
            fixture.lower_pose.fixed_face(),
            fixture.lower_pose.hinge_angles(),
            FIXTURE_CLOSURE_TOLERANCE_V2,
        )
        .unwrap();
    let foreign_upper = foreign_geometry
        .solve_closed(
            &fixture.audit,
            fixture.upper_pose.fixed_face(),
            fixture.upper_pose.hinge_angles(),
            FIXTURE_CLOSURE_TOLERANCE_V2,
        )
        .unwrap();
    let mut foreign_input = fixture.input_v2();
    foreign_input.geometry = &foreign_geometry;
    foreign_input.lower_pose = &foreign_lower;
    foreign_input.upper_pose = &foreign_upper;
    assert_eq!(
        evidence.revalidate_v2(foreign_input),
        Err(
            CycleScheduleRepresentationBoundaryPoseAngleIdentityErrorV2::CertificateBindingMismatch
        )
    );
}
