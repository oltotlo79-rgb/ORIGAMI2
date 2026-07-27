use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::FoldAssignment;

use super::test_support::*;
use super::*;
use crate::{CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1};

#[test]
fn generator_word_certifies_a_non_cactus_multi_generator_coboundary() {
    let fixture = generator_word_fixture_v1(false, false);
    assert_eq!(fixture.geometry.face_ids().len(), 12);
    assert_eq!(fixture.geometry.hinges().len(), 39);
    assert_eq!(fixture.audit.closure_hinges().len(), 28);
    assert_eq!(fixture.zero_edges.len(), 12);

    let first = fixture
        .geometry
        .hinges()
        .iter()
        .find(|hinge| hinge.edge() == fixture.moving_edges[0])
        .unwrap();
    let second = fixture
        .geometry
        .hinges()
        .iter()
        .find(|hinge| hinge.edge() == fixture.moving_edges[1])
        .unwrap();
    assert_ne!(first.start(), second.start());
    let (first_line, first_sign) = exact_generator_line_v1(first).unwrap();
    let (second_line, second_sign) = exact_generator_line_v1(second).unwrap();
    assert_eq!(first_line, second_line);
    assert_eq!(first_sign, second_sign);

    for schedule in [
        polynomial_schedule_v1(&fixture, ScheduleMutationV1::None),
        half_angle_schedule_v1(&fixture),
    ] {
        assert!(exact_generator_word_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }

    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    let certificate = fixture
        .geometry
        .prove_dyadic_schedule_closure_v1(
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 0,
                max_leaves: 1,
                max_work: 1,
                schedule_limits: CycleScheduleLimitsV1 {
                    max_hinges: 0,
                    max_degree: 0,
                    max_coefficient_bits: 1,
                    max_work: 0,
                },
            },
        )
        .expect("the exact word issuer must bypass interval subdivision");
    assert_eq!(certificate.leaves().len(), 1);
}

#[test]
fn generator_word_is_invariant_to_storage_order_and_hinge_reversal() {
    for (reverse_every_other, reverse_storage) in [(false, true), (true, false), (true, true)] {
        let fixture = generator_word_fixture_v1(reverse_every_other, reverse_storage);
        let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
        assert!(exact_generator_word_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            1.0e-9,
        ));
        // Regression-only numerical observations do not participate in the
        // exact word proof above.
        for parameter in [0.0, 0.25, 0.5, 1.0] {
            let angles = schedule.evaluate(parameter).unwrap();
            assert!(
                fixture
                    .geometry
                    .solve_closed(&fixture.audit, fixture.fixed_face, &angles, 1.0e-8)
                    .is_ok()
            );
        }
    }
}

fn inverse_word_square_v1(commutator: bool) -> ExactGeneratorWordFixtureV1 {
    let namespace = ProjectId::new();
    let faces = (0..4)
        .map(|index| FaceId::derive_v5(namespace, format!("word-square:{index}").as_bytes()))
        .collect::<Vec<_>>();
    let edges = (0..4)
        .map(|index| EdgeId::derive_v5(namespace, format!("word-square-edge:{index}").as_bytes()))
        .collect::<Vec<_>>();
    let x_start = Point3::new(0.0, 0.0, 0.0).unwrap();
    let x_end = Point3::new(1.0, 0.0, 0.0).unwrap();
    let x_axis = Point3::new(1.0, 0.0, 0.0).unwrap();
    let y_start = Point3::new(0.0, 0.0, 1.0).unwrap();
    let y_end = Point3::new(0.0, 1.0, 1.0).unwrap();
    let y_axis = Point3::new(0.0, 1.0, 0.0).unwrap();
    let third_uses_x = commutator;
    let hinges = vec![
        TreeHinge::new_for_test(
            edges[0],
            FoldAssignment::Mountain,
            faces[0],
            faces[1],
            x_start,
            x_end,
            x_axis,
        ),
        TreeHinge::new_for_test(
            edges[1],
            FoldAssignment::Mountain,
            faces[1],
            faces[2],
            y_start,
            y_end,
            y_axis,
        ),
        TreeHinge::new_for_test(
            edges[2],
            FoldAssignment::Valley,
            faces[2],
            faces[3],
            if third_uses_x { x_start } else { y_start },
            if third_uses_x { x_end } else { y_end },
            if third_uses_x { x_axis } else { y_axis },
        ),
        TreeHinge::new_for_test(
            edges[3],
            FoldAssignment::Valley,
            faces[3],
            faces[0],
            if third_uses_x { y_start } else { x_start },
            if third_uses_x { y_end } else { x_end },
            if third_uses_x { y_axis } else { x_axis },
        ),
    ];
    rebuild_fixture_v1(
        faces.clone(),
        hinges,
        ExactGeneratorWordFixturePartsV1 {
            fixed_face: faces[0],
            moving_edges: edges,
            constant_edges: Vec::new(),
            zero_edges: Vec::new(),
            groups: faces.into_iter().map(|face| vec![face]).collect(),
        },
        false,
    )
}

#[test]
fn generator_word_accepts_only_free_reduction_not_a_noncommuting_commutator() {
    let reducible = inverse_word_square_v1(false);
    let schedule = polynomial_schedule_v1(&reducible, ScheduleMutationV1::None);
    assert!(exact_generator_word_cycle_closure_premises_v1(
        &reducible.geometry,
        &reducible.audit,
        reducible.fixed_face,
        &schedule,
        0.0,
    ));

    let commutator = inverse_word_square_v1(true);
    let schedule = polynomial_schedule_v1(&commutator, ScheduleMutationV1::None);
    assert!(!exact_generator_word_cycle_closure_premises_v1(
        &commutator.geometry,
        &commutator.audit,
        commutator.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn generator_word_rejects_one_ulp_line_angle_and_profile_changes() {
    let fixture = generator_word_fixture_v1(false, false);
    let changed = fixture.moving_edges[0];
    let subnormal = f64::from_bits(1);
    let fixture = replace_hinge_v1(fixture, changed, |hinge| {
        TreeHinge::new_for_test(
            hinge.edge(),
            hinge.assignment(),
            hinge.left_face(),
            hinge.right_face(),
            Point3::new(hinge.start().x(), subnormal, hinge.start().z()).unwrap(),
            Point3::new(hinge.end().x(), subnormal, hinge.end().z()).unwrap(),
            hinge.axis(),
        )
    });
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(!exact_generator_word_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));

    let fixture = generator_word_fixture_v1(false, false);
    let changed = fixture.constant_edges[0];
    let schedule = polynomial_schedule_v1(
        &fixture,
        ScheduleMutationV1::ConstantAngle(changed, 30.0_f64.to_bits() + 1),
    );
    assert!(!exact_generator_word_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));

    let fixture = generator_word_fixture_v1(false, false);
    let changed = fixture.moving_edges[0];
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::MovingProfile(changed));
    assert!(!exact_generator_word_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));
}

#[test]
fn generator_word_rejects_assignment_and_axis_tamper() {
    let fixture = generator_word_fixture_v1(false, false);
    let changed = fixture.moving_edges[0];
    let fixture = replace_hinge_v1(fixture, changed, |hinge| {
        TreeHinge::new_for_test(
            hinge.edge(),
            FoldAssignment::Valley,
            hinge.left_face(),
            hinge.right_face(),
            hinge.start(),
            hinge.end(),
            hinge.axis(),
        )
    });
    let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
    assert!(!exact_generator_word_cycle_closure_premises_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &schedule,
        0.0,
    ));

    for overflow in [false, true] {
        let fixture = generator_word_fixture_v1(false, false);
        let changed = fixture.moving_edges[0];
        let fixture = replace_hinge_v1(fixture, changed, |hinge| {
            if overflow {
                TreeHinge::new_for_test(
                    hinge.edge(),
                    hinge.assignment(),
                    hinge.left_face(),
                    hinge.right_face(),
                    Point3::new(-f64::MAX, 0.0, 0.0).unwrap(),
                    Point3::new(f64::MAX, 0.0, 0.0).unwrap(),
                    Point3::new(1.0, 0.0, 0.0).unwrap(),
                )
            } else {
                TreeHinge::new_for_test(
                    hinge.edge(),
                    hinge.assignment(),
                    hinge.left_face(),
                    hinge.right_face(),
                    hinge.start(),
                    hinge.end(),
                    Point3::new(0.0, -0.0, 0.0).unwrap(),
                )
            }
        });
        let schedule = polynomial_schedule_v1(&fixture, ScheduleMutationV1::None);
        assert!(!exact_generator_word_cycle_closure_premises_v1(
            &fixture.geometry,
            &fixture.audit,
            fixture.fixed_face,
            &schedule,
            0.0,
        ));
    }
}

#[test]
fn generator_word_authentication_rejects_non_simple_and_audit_mismatch() {
    let fixture = generator_word_fixture_v1(false, false);
    let faces = fixture.geometry.face_ids().to_vec();
    let hinges = fixture.geometry.hinges().to_vec();
    let audit = fixture.audit.clone();

    let mut self_loop = hinges.clone();
    let hinge = self_loop[0].clone();
    self_loop[0] = TreeHinge::new_for_test(
        hinge.edge(),
        hinge.assignment(),
        hinge.left_face(),
        hinge.left_face(),
        hinge.start(),
        hinge.end(),
        hinge.axis(),
    );
    assert!(
        authenticate_graph_v1(
            &MaterialHingeGraphGeometry::new_for_test(faces.clone(), self_loop),
            &audit,
        )
        .is_none()
    );

    let mut duplicate_pair = hinges.clone();
    let first = duplicate_pair[0].clone();
    let second = duplicate_pair[1].clone();
    duplicate_pair[1] = TreeHinge::new_for_test(
        second.edge(),
        second.assignment(),
        first.left_face(),
        first.right_face(),
        second.start(),
        second.end(),
        second.axis(),
    );
    assert!(
        authenticate_graph_v1(
            &MaterialHingeGraphGeometry::new_for_test(faces.clone(), duplicate_pair),
            &audit,
        )
        .is_none()
    );

    let mut duplicate_edge = hinges.clone();
    let first = duplicate_edge[0].clone();
    let second = duplicate_edge[1].clone();
    duplicate_edge[1] = TreeHinge::new_for_test(
        first.edge(),
        second.assignment(),
        second.left_face(),
        second.right_face(),
        second.start(),
        second.end(),
        second.axis(),
    );
    assert!(
        authenticate_graph_v1(
            &MaterialHingeGraphGeometry::new_for_test(faces.clone(), duplicate_edge),
            &audit,
        )
        .is_none()
    );

    let missing = MaterialHingeGraphGeometry::new_for_test(
        faces.clone(),
        hinges[..hinges.len() - 1].to_vec(),
    );
    assert!(authenticate_graph_v1(&missing, &audit).is_none());

    let mut extra = hinges;
    extra.push(TreeHinge::new_for_test(
        EdgeId::derive_v5(ProjectId::new(), b"generator-word-extra"),
        FoldAssignment::Mountain,
        fixture.groups[0][0],
        fixture.groups[3][0],
        Point3::new(0.0, 20.0, 0.0).unwrap(),
        Point3::new(1.0, 20.0, 0.0).unwrap(),
        Point3::new(1.0, 0.0, 0.0).unwrap(),
    ));
    assert!(
        authenticate_graph_v1(
            &MaterialHingeGraphGeometry::new_for_test(faces, extra),
            &audit,
        )
        .is_none()
    );
}

#[test]
fn generator_word_constant_key_is_exact_and_uses_no_special_relations() {
    assert_eq!(exact_constant_profile_v1((-0.0_f64).to_bits()), Some(None));
    assert!(exact_constant_profile_v1((-1.0_f64).to_bits()).is_none());
    assert!(exact_constant_profile_v1(f64::NAN.to_bits()).is_none());
    assert!(exact_constant_profile_v1(f64::INFINITY.to_bits()).is_none());

    let ninety = exact_constant_profile_v1(90.0_f64.to_bits()).unwrap();
    let one_eighty = exact_constant_profile_v1(180.0_f64.to_bits()).unwrap();
    let below_one_eighty = exact_constant_profile_v1(180.0_f64.to_bits() - 1).unwrap();
    assert_ne!(ninety, one_eighty);
    assert_ne!(one_eighty, below_one_eighty);
}

#[test]
fn generator_line_sign_follows_the_live_left_to_right_transform_convention() {
    let fixture = generator_word_fixture_v1(false, false);
    let hinge = fixture
        .geometry
        .hinges()
        .iter()
        .find(|hinge| hinge.edge() == fixture.moving_edges[0])
        .unwrap();
    let reversed = TreeHinge::new_for_test(
        hinge.edge(),
        hinge.assignment(),
        hinge.right_face(),
        hinge.left_face(),
        hinge.end(),
        hinge.start(),
        Point3::new(-hinge.axis().x(), -hinge.axis().y(), -hinge.axis().z()).unwrap(),
    );
    let valley = TreeHinge::new_for_test(
        hinge.edge(),
        FoldAssignment::Valley,
        hinge.left_face(),
        hinge.right_face(),
        hinge.start(),
        hinge.end(),
        hinge.axis(),
    );
    let (line, sign) = exact_generator_line_v1(hinge).unwrap();
    let (reversed_line, reversed_sign) = exact_generator_line_v1(&reversed).unwrap();
    let (valley_line, valley_sign) = exact_generator_line_v1(&valley).unwrap();
    assert_eq!(line, reversed_line);
    assert_eq!(line, valley_line);
    assert_eq!(sign, -reversed_sign);
    assert_eq!(sign, -valley_sign);
}
