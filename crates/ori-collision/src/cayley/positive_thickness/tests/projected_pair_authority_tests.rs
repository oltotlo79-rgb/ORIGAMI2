//! Projected-pair authority and pair-local pipeline regression tests.

use super::*;

fn uniform_pose_with_root(
    model: &MaterialTreeKinematicsModel,
    angle_degrees: f64,
    root: FaceId,
) -> MaterialTreePose {
    let angles = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), angle_degrees).unwrap())
            .collect(),
    )
    .expect("canonical uniform angles");
    model
        .solve(Some(root), &angles)
        .expect("uniform native material pose")
}

fn symmetric_three_triangle_chain_model(namespace_index: u64) -> MaterialTreeKinematicsModel {
    unsupported_polygon_model(
        &[
            (0.0, 0.0),
            (0.0, -400.0),
            (400.0, 0.0),
            (0.0, 400.0),
            (-400.0, 0.0),
        ],
        &[(0, 2, EdgeKind::Mountain), (0, 3, EdgeKind::Valley)],
        namespace_index,
    )
}

#[test]
fn projected_three_face_pair_authority_rebinds_whole_parent_and_every_seal() {
    let model = three_triangle_chain_model(9_005);
    let pose = uniform_pose(&model, 30.0);
    let bound = model.bind_pose(&pose).expect("bound three-face pose");
    let exact = triangular_exact_pose(&model, &pose);
    let independently_regenerated_exact = triangular_exact_pose(&model, &pose);
    let same_angle_aba = uniform_pose(&model, 30.0);
    let aba_bound = model.bind_pose(&same_angle_aba).expect("bound ABA pose");

    for source_hinge in model.hinges() {
        let authority =
            prepare_projected_pair_authority_v1(&exact, bound, source_hinge.edge(), 0.1)
                .expect("3/2 pair authority");
        let scope = revalidate_projected_pair_authority_v1(&authority, &exact, bound, 0.1)
            .expect("same whole parent, pair, and bit seals");
        assert_eq!(scope.full_face_count(), 3);
        assert_eq!(scope.full_hinge_count(), 2);
        assert_eq!(scope.edge(), source_hinge.edge());
        assert_eq!(
            scope.face_ids(&exact),
            Some([source_hinge.left_face(), source_hinge.right_face()])
        );
        assert!(
            revalidate_projected_pair_authority_v1(
                &authority,
                &independently_regenerated_exact,
                bound,
                0.1,
            )
            .is_none(),
            "equal exact geometry from another object is not the retained issuer"
        );
        assert!(
            revalidate_projected_pair_authority_v1(&authority, &exact, aba_bound, 0.1).is_none(),
            "same-angle ABA pose must not reuse pair authority"
        );
        assert!(
            revalidate_projected_pair_authority_v1(&authority, &exact, bound, next_up(0.1))
                .is_none(),
            "one-ULP thickness drift must fail"
        );

        let mut transform_tampered =
            prepare_projected_pair_authority_v1(&exact, bound, source_hinge.edge(), 0.1)
                .expect("fresh transform authority");
        transform_tampered.flip_transform_bit_for_test(0, 0, 0);
        assert!(
            revalidate_projected_pair_authority_v1(&transform_tampered, &exact, bound, 0.1)
                .is_none()
        );

        let mut excluded_tampered =
            prepare_projected_pair_authority_v1(&exact, bound, source_hinge.edge(), 0.1)
                .expect("fresh excluded-set authority");
        excluded_tampered.clear_excluded_faces_for_test();
        assert!(
            revalidate_projected_pair_authority_v1(&excluded_tampered, &exact, bound, 0.1)
                .is_none()
        );

        let mut face_index_tampered =
            prepare_projected_pair_authority_v1(&exact, bound, source_hinge.edge(), 0.1)
                .expect("fresh face-index authority");
        face_index_tampered.replace_first_face_with_excluded_for_test();
        assert!(
            revalidate_projected_pair_authority_v1(&face_index_tampered, &exact, bound, 0.1)
                .is_none()
        );

        let mut hinge_index_tampered =
            prepare_projected_pair_authority_v1(&exact, bound, source_hinge.edge(), 0.1)
                .expect("fresh hinge-index authority");
        hinge_index_tampered.invalidate_hinge_index_for_test();
        assert!(
            revalidate_projected_pair_authority_v1(&hinge_index_tampered, &exact, bound, 0.1)
                .is_none()
        );

        let mut edge_tampered =
            prepare_projected_pair_authority_v1(&exact, bound, source_hinge.edge(), 0.1)
                .expect("fresh edge authority");
        edge_tampered.replace_edge_for_test(triangular_edge_id(999_999));
        assert!(
            revalidate_projected_pair_authority_v1(&edge_tampered, &exact, bound, 0.1).is_none()
        );

        let mut count_tampered =
            prepare_projected_pair_authority_v1(&exact, bound, source_hinge.edge(), 0.1)
                .expect("fresh count authority");
        count_tampered.increment_full_face_count_for_test();
        assert!(
            revalidate_projected_pair_authority_v1(&count_tampered, &exact, bound, 0.1).is_none()
        );

        let mut hinge_count_tampered =
            prepare_projected_pair_authority_v1(&exact, bound, source_hinge.edge(), 0.1)
                .expect("fresh hinge-count authority");
        hinge_count_tampered.increment_full_hinge_count_for_test();
        assert!(
            revalidate_projected_pair_authority_v1(&hinge_count_tampered, &exact, bound, 0.1)
                .is_none()
        );

        let mut angle_tampered =
            prepare_projected_pair_authority_v1(&exact, bound, source_hinge.edge(), 0.1)
                .expect("fresh angle authority");
        angle_tampered.increment_angle_bits_for_test();
        assert!(
            revalidate_projected_pair_authority_v1(&angle_tampered, &exact, bound, 0.1).is_none()
        );
    }
    assert!(
        prepare_projected_pair_authority_v1(&exact, bound, triangular_edge_id(999_999), 0.1,)
            .is_none()
    );
}

fn assert_projected_pipeline_case<'pose>(
    exact: &RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    root: FaceId,
    edge: EdgeId,
    expected_faces: [FaceId; 2],
    thickness: f64,
) {
    let prerequisite_analysis = analyze_single_triangular_hinge_prerequisites_for_edge_v1(
        exact,
        thickness,
        Some(edge),
        SingleTriangularHingePrerequisiteLimits::default(),
    )
    .expect("bounded projected prerequisite");
    let prerequisite = match &prerequisite_analysis.result {
        SingleTriangularHingePrerequisiteResult::Authenticated(prerequisite) => prerequisite,
        result => {
            panic!("prerequisite: root={root:?}, edge={edge:?}, thickness={thickness}: {result:?}")
        }
    };
    assert!(
        revalidate_single_triangular_hinge_prerequisites_v1(prerequisite, exact, thickness,)
            .is_some(),
        "prerequisite revalidation: root={root:?}, edge={edge:?}, thickness={thickness}"
    );

    let ef_analysis = analyze_axis_aligned_ef_boundary_v1(
        prerequisite,
        exact,
        bound,
        thickness,
        AxisAlignedEfBoundaryLimits::default(),
    )
    .expect("bounded projected E/F analysis");
    let ef = ef_analysis.capability.as_ref().unwrap_or_else(|| {
        panic!("E/F: root={root:?}, edge={edge:?}, thickness={thickness}: {ef_analysis:?}")
    });
    assert!(
        revalidate_axis_aligned_ef_boundary_v1(ef, prerequisite, exact, bound, thickness).is_some(),
        "E/F revalidation: root={root:?}, edge={edge:?}, thickness={thickness}"
    );

    let exact_e_analysis = analyze_exact_e_finite_hinge_corridor_v1(
        &prerequisite_analysis,
        Some(ef),
        exact,
        bound,
        thickness,
        ExactEFiniteHingeCorridorLimits::default(),
    )
    .expect("bounded projected exact-E analysis");
    let exact_e_contained = exact_e_analysis.authenticated_contained_capability_and_work();
    match &exact_e_analysis.result {
        ExactEFiniteHingeCorridorResult::Contained(_) => {
            let (capability, _) = exact_e_contained.expect("contained exact-E has sealed work");
            assert!(
                revalidate_exact_e_finite_hinge_corridor_v1(
                    capability,
                    prerequisite,
                    ef,
                    exact,
                    bound,
                    thickness,
                )
                .is_some(),
                "exact-E revalidation: root={root:?}, edge={edge:?}, thickness={thickness}"
            );
        }
        ExactEFiniteHingeCorridorResult::Outside(_) => {
            assert!(exact_e_contained.is_none());
        }
        result => {
            panic!("exact-E: root={root:?}, edge={edge:?}, thickness={thickness}: {result:?}")
        }
    }

    let direct_f_analysis = analyze_direct_f_finite_hinge_corridor_v1(
        &prerequisite_analysis,
        Some(ef),
        &exact_e_analysis,
        exact,
        bound,
        thickness,
        DirectFFiniteHingeCorridorLimits::default(),
    )
    .expect("bounded projected direct-F analysis");
    let direct_f_contained = direct_f_analysis.authenticated_contained_capability_and_work();
    match (&exact_e_analysis.result, &direct_f_analysis.result) {
        (
            ExactEFiniteHingeCorridorResult::Contained(_),
            DirectFFiniteHingeCorridorResult::Contained(_),
        ) => {
            let (exact_e, _) = exact_e_contained.expect("contained exact-E has sealed work");
            let (capability, _) = direct_f_contained.expect("contained direct-F has sealed work");
            assert!(
                revalidate_direct_f_finite_hinge_corridor_v1(
                    capability,
                    prerequisite,
                    ef,
                    exact_e,
                    exact,
                    bound,
                    thickness,
                )
                .is_some(),
                "direct-F revalidation: root={root:?}, edge={edge:?}, thickness={thickness}"
            );
        }
        (
            ExactEFiniteHingeCorridorResult::Contained(_),
            DirectFFiniteHingeCorridorResult::Outside(_),
        )
        | (
            ExactEFiniteHingeCorridorResult::Outside(_),
            DirectFFiniteHingeCorridorResult::Unresolved,
        ) => {
            assert!(direct_f_contained.is_none());
        }
        (_, result) => panic!(
            "direct-F: root={root:?}, edge={edge:?}, thickness={thickness}: \
             exact-E={:?}, direct-F={result:?}",
            exact_e_analysis.result
        ),
    }

    let admission_analysis = analyze_shared_hinge_corridor_admission_v1(
        &prerequisite_analysis,
        Some(ef),
        &exact_e_analysis,
        &direct_f_analysis,
        exact,
        bound,
        thickness,
        SharedHingeCorridorAdmissionLimitsV1::default(),
    )
    .expect("bounded projected admission analysis");
    let margin_analysis = analyze_shared_hinge_native_exact_topology_margin_v1(
        &prerequisite_analysis,
        Some(ef),
        exact,
        bound,
        thickness,
        SharedHingeNativeExactTopologyMarginLimitsV1::default(),
    )
    .expect("bounded projected margin analysis");
    let (margin, _) = margin_analysis
        .authenticated_capability_and_work()
        .unwrap_or_else(|| {
            panic!(
                "margin: root={root:?}, edge={edge:?}, thickness={thickness}: \
                 admission={:?}, margin={:?}",
                admission_analysis.result, margin_analysis.result
            )
        });
    assert!(
        revalidate_shared_hinge_native_exact_topology_margin_v1(
            margin,
            prerequisite,
            ef,
            exact,
            bound,
            thickness,
        )
        .is_some(),
        "margin revalidation: root={root:?}, edge={edge:?}, thickness={thickness}"
    );

    let classification = analyze_shared_hinge_solid_classification_v1(
        &prerequisite_analysis,
        Some(ef),
        &exact_e_analysis,
        &direct_f_analysis,
        &admission_analysis,
        &margin_analysis,
        exact,
        bound,
        thickness,
        SharedHingeSolidClassificationLimitsV1::default(),
    )
    .expect("bounded projected classification");
    assert!(
        matches!(
            &classification.result,
            SharedHingeSolidClassificationResultV1::Classified(_)
                | SharedHingeSolidClassificationResultV1::IndependentlyClassified(_)
        ),
        "classification: root={:?}, edge={:?}, thickness={}: \
         exact-E={:?}, direct-F={:?}, result={:?}",
        root,
        edge,
        thickness,
        exact_e_analysis.result,
        direct_f_analysis.result,
        classification.result
    );

    let summary = diagnose_bound_shared_hinge_solid_for_edge_v1(bound, thickness, Some(edge))
        .expect("bounded pair-local classifier")
        .expect("supported 3/2 pair-local diagnostic");
    assert_eq!(
        summary.disposition,
        SharedHingeSolidDiagnosticDispositionV1::Allowed,
        "root={root:?}, edge={edge:?}, thickness={thickness}"
    );
    let mut actual_faces = [summary.first_face, summary.second_face];
    actual_faces.sort_unstable_by_key(FaceId::canonical_bytes);
    let mut expected_faces = expected_faces;
    expected_faces.sort_unstable_by_key(FaceId::canonical_bytes);
    assert_eq!(actual_faces, expected_faces);
}

#[test]
fn projected_three_face_lower_pipeline_classifies_each_hinge_without_whole_pose_authority() {
    let model = symmetric_three_triangle_chain_model(9_006);
    assert_eq!(model.face_ids().len(), 3);
    assert_eq!(model.hinges().len(), 2);
    for root in model.face_ids() {
        let pose = uniform_pose_with_root(&model, 10.0, *root);
        let bound = model.bind_pose(&pose).expect("bound 3/2 root pose");
        let exact = triangular_exact_pose(&model, &pose);
        for thickness in [0.1, 1.0, 3.0] {
            for source_hinge in model.hinges() {
                assert_projected_pipeline_case(
                    &exact,
                    bound,
                    *root,
                    source_hinge.edge(),
                    [source_hinge.left_face(), source_hinge.right_face()],
                    thickness,
                );
            }
        }
    }
}

#[test]
fn projected_pair_authority_keeps_four_face_parent_fail_closed() {
    let model = unsupported_polygon_model(
        &[
            (0.0, 0.0),
            (300.0, 0.0),
            (450.0, 200.0),
            (300.0, 400.0),
            (0.0, 400.0),
            (-150.0, 200.0),
        ],
        &[
            (0, 2, EdgeKind::Mountain),
            (0, 3, EdgeKind::Valley),
            (0, 4, EdgeKind::Mountain),
        ],
        9_007,
    );
    assert_eq!(model.face_ids().len(), 4);
    assert_eq!(model.hinges().len(), 3);
    let pose = uniform_pose(&model, 30.0);
    let bound = model.bind_pose(&pose).expect("bound 4/3 pose");
    for source_hinge in model.hinges() {
        let diagnostic =
            diagnose_bound_shared_hinge_solid_for_edge_v1(bound, 0.1, Some(source_hinge.edge()))
                .expect("unsupported parent remains bounded");
        assert!(diagnostic.is_none_or(|summary| {
            summary.disposition == SharedHingeSolidDiagnosticDispositionV1::Indeterminate
        }));
    }
}

#[test]
fn projected_three_face_boundary_and_unspecified_edge_remain_fail_closed() {
    let model = three_triangle_chain_model(9_008);
    let pose = uniform_pose(&model, 90.0);
    let bound = model.bind_pose(&pose).expect("bound 90-degree 3/2 pose");
    assert!(
        diagnose_bound_shared_hinge_solid_v1(bound, 0.1)
            .expect("unspecified multi-hinge call remains bounded")
            .is_none()
    );
    assert!(
        diagnose_bound_shared_hinge_solid_for_edge_v1(
            bound,
            0.1,
            Some(triangular_edge_id(999_999)),
        )
        .expect("unknown edge remains bounded")
        .is_none()
    );
    for source_hinge in model.hinges() {
        let summary =
            diagnose_bound_shared_hinge_solid_for_edge_v1(bound, 0.1, Some(source_hinge.edge()))
                .expect("90-degree boundary remains bounded")
                .expect("known pair has an explicit fail-closed diagnostic");
        assert_eq!(
            summary.disposition,
            SharedHingeSolidDiagnosticDispositionV1::Indeterminate
        );
    }
}
