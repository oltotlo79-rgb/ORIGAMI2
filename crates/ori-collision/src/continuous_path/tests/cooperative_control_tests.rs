use super::*;

#[test]
fn controlled_path_pre_cancelled_issues_no_diagnostic_or_certificate() {
    let model = one_hinge_model();
    let edge = model.hinges()[0].edge();
    let source = CanonicalHingeAngles::new(vec![HingeAngle::new(edge, 0.0).unwrap()]).unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &source).unwrap();
    let target = CanonicalHingeAngles::new(vec![HingeAngle::new(edge, 45.0).unwrap()]).unwrap();
    let cancelled = std::sync::atomic::AtomicBool::new(true);
    let control = CooperativeOperationControlV1::new(
        Some(&cancelled),
        std::time::Instant::now() + std::time::Duration::from_secs(1),
    );

    assert_eq!(
        diagnose_collective_hinge_path_from_pose_with_control_v1(
            &model,
            &pose,
            pose.hinge_angles(),
            target.as_slice(),
            0.0,
            StackedFoldPathDiagnosticLimitsV1::default(),
            &control,
        ),
        Err(StackedFoldPathDiagnosticErrorV1::Cancelled)
    );
    assert!(matches!(
        certify_tree_continuous_path_from_pose_with_control_v1(
            &model,
            &pose,
            &target,
            0.0,
            StackedFoldPathDiagnosticLimitsV1::default(),
            &control,
        ),
        Err(StackedFoldPathDiagnosticErrorV1::Cancelled)
    ));
}
