//! General material-tree projection and one-exact-session regressions.

use super::super::projected_pair_authority::{
    ProjectedPairAuthorityLimitsV1, checked_excluded_face_index_binding_count,
    prepare_projected_pair_authority_with_limits_v1, projected_tree_counts_fit_limits_v1,
};
use super::super::shared_hinge_pair_session::{
    prepare_shared_hinge_pair_diagnostic_session_v1,
    prepare_shared_hinge_pair_diagnostic_session_with_limits_v1,
};
use super::*;

fn triangle_fan_model(face_count: usize, namespace_index: u64) -> MaterialTreeKinematicsModel {
    let coordinates = (0..face_count + 2)
        .map(|index| {
            let x = index as f64 * 20.0;
            (x, x * x / 400.0)
        })
        .collect::<Vec<_>>();
    let creases = (2..coordinates.len() - 1)
        .map(|index| (0, index, EdgeKind::Mountain))
        .collect::<Vec<_>>();
    let model = unsupported_polygon_model(&coordinates, &creases, namespace_index);
    assert_eq!(model.face_ids().len(), face_count);
    assert_eq!(model.hinges().len(), face_count - 1);
    model
}

fn uniform_tree_pose(
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
    .expect("canonical general-tree angles");
    model
        .solve(Some(root), &angles)
        .expect("general-tree material pose")
}

#[test]
fn four_eight_and_sixteen_face_sessions_classify_every_pair_locally() {
    for (case, face_count, thickness, reverse_root) in [
        (0_u64, 4_usize, 0.1, false),
        (1, 8, 1.0, true),
        (2, 16, 3.0, false),
    ] {
        let model = triangle_fan_model(face_count, 9_100 + case);
        let root = if reverse_root {
            *model.face_ids().last().unwrap()
        } else {
            model.face_ids()[0]
        };
        let pose = uniform_tree_pose(&model, 0.0, root);
        let bound = model.bind_pose(&pose).expect("bound general-tree pose");
        let session = prepare_shared_hinge_pair_diagnostic_session_v1(bound, thickness)
            .expect("bounded session")
            .expect("supported general tree");
        assert_eq!(session.full_face_count_for_test(), face_count);
        assert_eq!(session.full_hinge_count_for_test(), face_count - 1);
        assert_eq!(
            session.excluded_face_index_bindings_for_test(),
            (face_count - 1) * (face_count - 2)
        );
        let exact = session.exact_for_test();
        for hinge in model.hinges() {
            assert!(std::ptr::eq(session.exact_for_test(), exact));
            let summary = session
                .diagnose(Some(hinge.edge()))
                .expect("bounded pair-local diagnostic")
                .expect("known general-tree hinge");
            assert_eq!(
                summary.disposition,
                SharedHingeSolidDiagnosticDispositionV1::Allowed,
                "faces={face_count}, edge={:?}",
                hinge.edge()
            );
            let mut actual = [summary.first_face, summary.second_face];
            actual.sort_unstable_by_key(FaceId::canonical_bytes);
            let mut expected = [hinge.left_face(), hinge.right_face()];
            expected.sort_unstable_by_key(FaceId::canonical_bytes);
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn general_tree_authority_rejects_every_issuer_and_excluded_set_tamper() {
    let model = triangle_fan_model(4, 9_110);
    let pose = uniform_tree_pose(&model, 0.0, model.face_ids()[0]);
    let bound = model.bind_pose(&pose).expect("bound four-face pose");
    let session = prepare_shared_hinge_pair_diagnostic_session_v1(bound, 0.1)
        .expect("bounded session")
        .expect("supported four-face tree");
    let exact = session.exact_for_test();
    let hinge = &model.hinges()[1];

    let authority = prepare_projected_pair_authority_v1(exact, bound, hinge.edge(), 0.1)
        .expect("four-face pair authority");
    let scope =
        revalidate_projected_pair_authority_v1(&authority, exact, bound, 0.1).expect("same issuer");
    assert_eq!(scope.full_face_count(), 4);
    assert_eq!(scope.full_hinge_count(), 3);
    assert_eq!(checked_excluded_face_index_binding_count(4, 3), Some(6));

    let independently_regenerated_exact = triangular_exact_pose(&model, &pose);
    assert!(
        revalidate_projected_pair_authority_v1(
            &authority,
            &independently_regenerated_exact,
            bound,
            0.1,
        )
        .is_none()
    );
    let aba_pose = uniform_tree_pose(&model, 0.0, model.face_ids()[0]);
    let aba_bound = model.bind_pose(&aba_pose).expect("bound ABA pose");
    assert!(revalidate_projected_pair_authority_v1(&authority, exact, aba_bound, 0.1).is_none());
    let rerooted_pose = uniform_tree_pose(&model, 0.0, model.face_ids()[3]);
    let rerooted_bound = model
        .bind_pose(&rerooted_pose)
        .expect("bound rerooted pose");
    assert!(
        revalidate_projected_pair_authority_v1(&authority, exact, rerooted_bound, 0.1).is_none()
    );
    let one_ulp_pose = uniform_tree_pose(&model, f64::from_bits(1), model.face_ids()[0]);
    let one_ulp_bound = model.bind_pose(&one_ulp_pose).expect("bound one-ULP pose");
    assert!(
        revalidate_projected_pair_authority_v1(&authority, exact, one_ulp_bound, 0.1).is_none()
    );
    assert!(
        revalidate_projected_pair_authority_v1(&authority, exact, bound, next_up(0.1)).is_none()
    );

    let foreign = triangle_fan_model(4, 9_110);
    assert_eq!(foreign.face_ids(), model.face_ids());
    let foreign_pose = uniform_tree_pose(&foreign, 0.0, foreign.face_ids()[0]);
    let foreign_bound = foreign
        .bind_pose(&foreign_pose)
        .expect("bound same-ID foreign pose");
    let foreign_exact = triangular_exact_pose(&foreign, &foreign_pose);
    assert!(
        revalidate_projected_pair_authority_v1(&authority, &foreign_exact, foreign_bound, 0.1,)
            .is_none()
    );

    macro_rules! assert_mutation_rejected {
        ($method:ident) => {{
            let mut tampered = prepare_projected_pair_authority_v1(exact, bound, hinge.edge(), 0.1)
                .expect("fresh authority");
            tampered.$method();
            assert!(
                revalidate_projected_pair_authority_v1(&tampered, exact, bound, 0.1).is_none(),
                stringify!($method)
            );
        }};
    }
    assert_mutation_rejected!(clear_excluded_faces_for_test);
    assert_mutation_rejected!(remove_last_excluded_face_for_test);
    assert_mutation_rejected!(duplicate_first_excluded_face_for_test);
    assert_mutation_rejected!(reverse_excluded_faces_for_test);
    assert_mutation_rejected!(append_selected_face_to_excluded_for_test);
    assert_mutation_rejected!(append_out_of_range_excluded_face_for_test);
    assert_mutation_rejected!(replace_first_face_with_excluded_for_test);
    assert_mutation_rejected!(swap_selected_faces_for_test);
    assert_mutation_rejected!(invalidate_hinge_index_for_test);
    assert_mutation_rejected!(increment_full_face_count_for_test);
    assert_mutation_rejected!(increment_full_hinge_count_for_test);
    assert_mutation_rejected!(increment_excluded_binding_work_for_test);
    assert_mutation_rejected!(increment_angle_bits_for_test);

    let mut transform_tampered =
        prepare_projected_pair_authority_v1(exact, bound, hinge.edge(), 0.1)
            .expect("fresh transform authority");
    transform_tampered.flip_transform_bit_for_test(0, 0, 0);
    assert!(
        revalidate_projected_pair_authority_v1(&transform_tampered, exact, bound, 0.1,).is_none()
    );
    let mut edge_tampered = prepare_projected_pair_authority_v1(exact, bound, hinge.edge(), 0.1)
        .expect("fresh edge authority");
    edge_tampered.replace_edge_for_test(triangular_edge_id(999_998));
    assert!(revalidate_projected_pair_authority_v1(&edge_tampered, exact, bound, 0.1).is_none());
}

#[test]
fn general_tree_projection_limits_cover_exact_one_short_hard_cap_and_overflow() {
    let model = triangle_fan_model(4, 9_120);
    let pose = uniform_tree_pose(&model, 0.0, model.face_ids()[0]);
    let bound = model.bind_pose(&pose).expect("bound four-face pose");
    let expected_bindings = 6;
    let exact_limits = ProjectedPairAuthorityLimitsV1 {
        max_parent_faces: 4,
        max_parent_hinges: 3,
        max_excluded_face_indexes: 2,
        max_excluded_face_index_bindings: expected_bindings,
    };
    let exact_session =
        prepare_shared_hinge_pair_diagnostic_session_with_limits_v1(bound, 0.1, exact_limits)
            .expect("exact limits")
            .expect("exact limits admit tree");
    assert_eq!(
        exact_session.excluded_face_index_bindings_for_test(),
        expected_bindings
    );

    let mut one_short = exact_limits;
    one_short.max_excluded_face_index_bindings = expected_bindings - 1;
    assert_eq!(
        prepare_shared_hinge_pair_diagnostic_session_with_limits_v1(bound, 0.1, one_short)
            .unwrap_err(),
        SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded
    );
    let mut face_one_short = exact_limits;
    face_one_short.max_parent_faces = 3;
    assert_eq!(
        prepare_shared_hinge_pair_diagnostic_session_with_limits_v1(bound, 0.1, face_one_short,)
            .unwrap_err(),
        SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded
    );
    let mut excluded_one_short = exact_limits;
    excluded_one_short.max_excluded_face_indexes = 1;
    assert!(
        prepare_projected_pair_authority_with_limits_v1(
            exact_session.exact_for_test(),
            bound,
            model.hinges()[0].edge(),
            0.1,
            excluded_one_short,
        )
        .is_none()
    );
    assert_eq!(
        projected_tree_counts_fit_limits_v1(4, 3, exact_limits),
        Some(expected_bindings)
    );
    assert_eq!(
        checked_excluded_face_index_binding_count(64, 63),
        Some(3_906)
    );
    assert!(checked_excluded_face_index_binding_count(usize::MAX, usize::MAX - 1).is_none());

    let over_hard_cap = triangle_fan_model(MAX_COMPOSED_THICKNESS_HINGES_V1 + 2, 9_121);
    let over_pose = uniform_tree_pose(&over_hard_cap, 0.0, over_hard_cap.face_ids()[0]);
    let over_bound = over_hard_cap
        .bind_pose(&over_pose)
        .expect("bound hard-cap-plus-one pose");
    let widened = ProjectedPairAuthorityLimitsV1 {
        max_parent_faces: usize::MAX,
        max_parent_hinges: usize::MAX,
        max_excluded_face_indexes: usize::MAX,
        max_excluded_face_index_bindings: usize::MAX,
    };
    assert_eq!(
        prepare_shared_hinge_pair_diagnostic_session_with_limits_v1(over_bound, 0.1, widened,)
            .unwrap_err(),
        SharedHingeSolidDiagnosticErrorV1::ResourceLimitExceeded
    );
}
