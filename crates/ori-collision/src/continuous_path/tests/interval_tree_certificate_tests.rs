use super::*;

#[test]
fn limits_fail_closed_before_geometry_access() {
    assert_eq!(MAX_STACKED_FOLD_PATH_SAMPLES_V1, 64);
    assert_eq!(
        StackedFoldPathDiagnosticLimitsV1::default().sample_intervals,
        8
    );
    assert_eq!(
        StackedFoldBoundedPathDiagnosticV1 {
            sampled_pose_count: 9,
            sampled_nonblocking_pose_count: 9,
            first_sampled_blocking_angle_degrees: None,
            requested_angle_degrees: 90.0,
            analytic_single_hinge_clearance: false,
            analytic_collinear_tree_clearance: false,
            analytic_positive_two_hinge_clearance: false,
            interval_two_hinge_chain_clearance: false,
            interval_tree_hinge_count: 0,
            interval_leaf_count: 0,
            interval_pair_work: 0,
            positive_endpoint_memo_pair_entries: 0,
            positive_endpoint_exact_pair_calls: 0,
            positive_thickness_outer_shell: false,
        }
        .safe_stop_angle_degrees()
        .to_bits(),
        0.0_f64.to_bits()
    );
}

#[test]
fn collinear_tree_gate_requires_one_exact_infinite_axis() {
    let origin = ori_kinematics::Point3::new(0.0, 0.0, 0.0).unwrap();
    let axis = ori_kinematics::Point3::new(1.0, 0.0, 0.0).unwrap();
    assert!(exact_collinear_line(
        origin,
        axis,
        ori_kinematics::Point3::new(4.0, 0.0, 0.0).unwrap(),
        ori_kinematics::Point3::new(-1.0, 0.0, 0.0).unwrap(),
    ));
    assert!(!exact_collinear_line(
        origin,
        axis,
        ori_kinematics::Point3::new(4.0, f64::from_bits(1), 0.0).unwrap(),
        ori_kinematics::Point3::new(1.0, 0.0, 0.0).unwrap(),
    ));
    assert!(!exact_collinear_line(
        origin,
        axis,
        origin,
        ori_kinematics::Point3::new(1.0, f64::from_bits(1), 0.0).unwrap(),
    ));
}

#[test]
fn separated_two_hinge_strip_gets_interval_clearance_certificate() {
    let model = two_hinge_strip_model();
    assert_eq!(model.face_ids().len(), 3);
    assert_eq!(model.hinges().len(), 2);
    let middle = model
        .face_ids()
        .iter()
        .copied()
        .find(|face| {
            model
                .hinges()
                .iter()
                .filter(|hinge| hinge.left_face() == *face || hinge.right_face() == *face)
                .count()
                == 2
        })
        .unwrap();
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(middle), &angles).unwrap();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let result = diagnose_collective_hinge_path_v1(
        &model,
        &pose,
        &moving,
        10.0,
        0.0,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(result.continuous_clearance_certified());
    assert_eq!(
        result.continuous_certificate_model_id(),
        Some(STACKED_FOLD_TWO_HINGE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
    );
    assert_eq!(result.safe_stop_angle_degrees(), 10.0);
}

#[test]
fn opaque_tree_certificate_revalidates_only_the_exact_admission_free_path() {
    let model = one_hinge_model();
    let edge = model.hinges()[0].edge();
    let source = CanonicalHingeAngles::new(vec![HingeAngle::new(edge, 0.0).unwrap()]).unwrap();
    let source_pose = model
        .solve(Some(model.face_ids()[0]), &source)
        .expect("exact flat source pose");
    let requested = CanonicalHingeAngles::new(vec![HingeAngle::new(edge, 37.0).unwrap()]).unwrap();
    let limits = StackedFoldPathDiagnosticLimitsV1::default();
    let certificate =
        certify_tree_continuous_path_from_pose_v1(&model, &source_pose, &requested, 0.0, limits)
            .expect("admission-free diagnosis")
            .expect("single-hinge analytic certificate");

    assert!(certificate.is_for(&model, &source_pose, &requested, 0.0));
    assert!(!certificate.authorizes_project_mutation());

    let target_one_ulp = CanonicalHingeAngles::new(vec![
        HingeAngle::new(edge, f64::from_bits(37.0_f64.to_bits() + 1)).unwrap(),
    ])
    .unwrap();
    assert!(!certificate.is_for(&model, &source_pose, &target_one_ulp, 0.0));

    let source_one_ulp = CanonicalHingeAngles::new(vec![
        HingeAngle::new(edge, f64::from_bits(0.0_f64.to_bits() + 1)).unwrap(),
    ])
    .unwrap();
    let source_pose_one_ulp = model
        .solve(Some(model.face_ids()[0]), &source_one_ulp)
        .expect("one-ULP source pose");
    assert!(!certificate.is_for(&model, &source_pose_one_ulp, &requested, 0.0));
    assert!(!certificate.is_for(
        &model,
        &source_pose,
        &requested,
        f64::from_bits(0.0_f64.to_bits() + 1),
    ));

    let foreign_model = one_hinge_model();
    assert!(!certificate.is_for(&foreign_model, &source_pose, &requested, 0.0));
}

#[test]
fn canonical_sweep_matches_bruteforce_for_single_nonadjacent_pair() {
    for (model, expected) in [
        (three_hinge_strip_model(false), true),
        (three_hinge_strip_model(true), false),
    ] {
        let angles = CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
        let moving = model
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<HashSet<_>>();
        let mut metrics = (0, 0);
        // For this four-face chain the exhaustive oracle has exactly the
        // three non-adjacent pairs; the established fixtures fix their
        // expected conjunction.
        assert_eq!(
            two_hinge_interval_clearance_premises(
                &model,
                &pose,
                &moving,
                if expected { 0.1 } else { 10.0 },
                8,
                &mut metrics,
                &CooperativeOperationControlV1::unbounded(),
            ),
            expected
        );
    }
}

#[test]
fn separated_three_hinge_tree_gets_bounded_interval_certificate() {
    let model = three_hinge_strip_model(false);
    let fixed = model
        .face_ids()
        .iter()
        .copied()
        .find(|face| {
            model
                .hinges()
                .iter()
                .filter(|hinge| hinge.left_face() == *face || hinge.right_face() == *face)
                .count()
                == 2
        })
        .unwrap();
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(fixed), &angles).unwrap();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let result = diagnose_collective_hinge_path_v1(
        &model,
        &pose,
        &moving,
        5.0,
        0.0,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(result.continuous_clearance_certified());
    assert_eq!(
        result.continuous_certificate_model_id(),
        Some(STACKED_FOLD_TREE_INTERVAL_CONTINUOUS_CERTIFICATE_MODEL_ID_V1)
    );
}

#[test]
fn absolute_collective_path_binds_the_complete_source_pose() {
    let model = three_hinge_strip_model(false);
    let (moving, zero_pose) = zero_tree_pose(&model);
    let source = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 1.0).unwrap())
            .collect(),
    )
    .unwrap();
    let source_pose = model.solve(zero_pose.fixed_face(), &source).unwrap();
    let target = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 2.0).unwrap())
            .collect(),
    )
    .unwrap();
    let result = diagnose_collective_hinge_path_from_pose_v1(
        &model,
        &source_pose,
        source.as_slice(),
        target.as_slice(),
        0.0,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(result.requested_angle_degrees(), 2.0);
    assert_eq!(
        diagnose_collective_hinge_path_from_pose_v1(
            &model,
            &source_pose,
            zero_pose.hinge_angles(),
            target.as_slice(),
            0.0,
            StackedFoldPathDiagnosticLimitsV1::default(),
        ),
        Err(StackedFoldPathDiagnosticErrorV1::PoseIssuerMismatch)
    );
    assert_eq!(
        diagnose_collective_hinge_path_v1(
            &model,
            &source_pose,
            &moving,
            2.0,
            0.0,
            StackedFoldPathDiagnosticLimitsV1::default(),
        ),
        Err(StackedFoldPathDiagnosticErrorV1::InvalidPath)
    );
}

#[test]
fn near_collision_three_hinge_tree_fails_closed() {
    let model = three_hinge_strip_model(true);
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &angles).unwrap();
    let moving = model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let result = diagnose_collective_hinge_path_v1(
        &model,
        &pose,
        &moving,
        10.0,
        0.0,
        StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .unwrap();
    assert!(!result.continuous_clearance_certified());
    assert_eq!(result.safe_stop_angle_degrees(), 0.0);
    let requested = CanonicalHingeAngles::new(
        moving
            .iter()
            .map(|edge| HingeAngle::new(*edge, 10.0).unwrap())
            .collect(),
    )
    .unwrap();
    assert!(
        certify_tree_continuous_path_from_pose_v1(
            &model,
            &pose,
            &requested,
            0.0,
            StackedFoldPathDiagnosticLimitsV1::default(),
        )
        .expect("bounded admission-free diagnosis")
        .is_none(),
        "an uncertified diagnostic must never mint typed evidence"
    );
}
