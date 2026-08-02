use super::*;

fn oriented_generalized_cut_fixture() -> ExactParallelCutFixture {
    let namespace = ProjectId::schema_namespace([
        0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d, 0x9e, 0x9f,
        0xa0,
    ]);
    let faces = (0..5)
        .map(|index| {
            FaceId::derive_v5(
                namespace,
                format!("generalized-cut-face-{index}").as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    let stationary = EdgeId::derive_v5(namespace, b"generalized-cut-stationary");
    let mut moving_edges = (0..4)
        .map(|index| {
            EdgeId::derive_v5(
                namespace,
                format!("generalized-cut-moving-{index}").as_bytes(),
            )
        })
        .collect::<Vec<_>>();
    moving_edges.sort_unstable_by_key(EdgeId::canonical_bytes);

    let plus_x = Point3::new(1.0, -0.0, 0.0).unwrap();
    let minus_x = Point3::new(-1.0, 0.0, -0.0).unwrap();
    let forward_start = Point3::new(0.0, 2.0, -3.0).unwrap();
    let forward_end = Point3::new(1.0, 2.0, -3.0).unwrap();
    let reverse_start = forward_end;
    let reverse_end = forward_start;
    let stationary_start = Point3::new(0.0, 0.0, 0.0).unwrap();
    let stationary_end = Point3::new(1.0, 0.0, 0.0).unwrap();

    // The least canonical moving edge starts on the sole central stationary
    // component, fixing the recognizer's deterministic reference side. The
    // remaining carriers exercise a parallel edge, reversed face traversal,
    // reversed endpoints/axis, and both assignments while preserving the same
    // effective central-to-outer directed line.
    let specifications = vec![
        (
            stationary,
            FoldAssignment::Mountain,
            faces[0],
            faces[1],
            stationary_start,
            stationary_end,
            plus_x,
        ),
        (
            moving_edges[0],
            FoldAssignment::Mountain,
            faces[0],
            faces[2],
            forward_start,
            forward_end,
            plus_x,
        ),
        (
            moving_edges[1],
            FoldAssignment::Mountain,
            faces[2],
            faces[0],
            reverse_start,
            reverse_end,
            minus_x,
        ),
        (
            moving_edges[2],
            FoldAssignment::Valley,
            faces[1],
            faces[3],
            reverse_start,
            reverse_end,
            minus_x,
        ),
        (
            moving_edges[3],
            FoldAssignment::Valley,
            faces[4],
            faces[1],
            forward_start,
            forward_end,
            plus_x,
        ),
    ];
    let triples = specifications
        .iter()
        .map(|(edge, _, left, right, ..)| (*edge, *left, *right))
        .collect::<Vec<_>>();
    let audit = MaterialHingeGraphAudit::prepare(
        &topology(&faces, &triples),
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    let geometry = MaterialHingeGraphGeometry::new_for_test(
        audit.faces().to_vec(),
        specifications
            .into_iter()
            .map(|(edge, assignment, left, right, start, end, axis)| {
                TreeHinge::new_for_test(edge, assignment, left, right, start, end, axis)
            })
            .collect(),
    );
    let mut canonical_edges = geometry
        .hinges()
        .iter()
        .map(TreeHinge::edge)
        .collect::<Vec<_>>();
    canonical_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let mut canonical_hinge_indices = (0..geometry.hinges().len()).collect::<Vec<_>>();
    canonical_hinge_indices
        .sort_unstable_by_key(|index| geometry.hinges()[*index].edge().canonical_bytes());
    let schedule_limits = CycleScheduleLimitsV1 {
        max_hinges: canonical_edges.len(),
        max_degree: 1,
        max_coefficient_bits: 53,
        max_work: 16_384,
    };
    let zero = RationalCoefficientV1 {
        numerator: 0,
        denominator: 1,
    };
    let slope = RationalCoefficientV1 {
        numerator: 1,
        denominator: 2,
    };
    let fixed_face = audit.faces()[0];
    let schedule = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed_face,
        [-1.0, 1.0],
        canonical_edges
            .iter()
            .map(|edge| CycleScheduleEntryInputV1 {
                edge: *edge,
                initial_angle_degrees_bits: if moving_edges.contains(edge) {
                    90.0_f64.to_bits()
                } else {
                    0.0_f64.to_bits()
                },
                chebyshev_coefficients: if moving_edges.contains(edge) {
                    vec![zero, slope]
                } else {
                    vec![zero]
                },
            })
            .collect(),
        schedule_limits,
    )
    .unwrap();

    ExactParallelCutFixture {
        geometry,
        audit,
        fixed_face,
        canonical_hinge_indices,
        canonical_edges,
        moving_edges,
        internal_edges: vec![stationary],
        schedule,
        schedule_limits,
    }
}

fn recognize(
    _fixture: &ExactParallelCutFixture,
    geometry: &MaterialHingeGraphGeometry,
    schedule: &CanonicalCycleScheduleV1,
    canonical_indices: &[usize],
    canonical_edges: &[EdgeId],
) -> Result<ExactParallelCutRecognitionV2, IntervalAttemptErrorV2> {
    recognize_exact_parallel_cut_with_checkpoint_v2(
        geometry,
        schedule,
        canonical_indices,
        canonical_edges,
        1_000_000,
        1_000_000,
        &mut || Ok(()),
    )
}

fn schedule_with_profile_override(
    fixture: &ExactParallelCutFixture,
    target: EdgeId,
    target_initial: f64,
    target_coefficients: Vec<RationalCoefficientV1>,
    max_degree: usize,
) -> CanonicalCycleScheduleV1 {
    let zero = RationalCoefficientV1 {
        numerator: 0,
        denominator: 1,
    };
    let slope = RationalCoefficientV1 {
        numerator: 1,
        denominator: 2,
    };
    CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        [-1.0, 1.0],
        fixture
            .canonical_edges
            .iter()
            .map(|edge| {
                let moving = fixture.moving_edges.contains(edge);
                CycleScheduleEntryInputV1 {
                    edge: *edge,
                    initial_angle_degrees_bits: if *edge == target {
                        target_initial.to_bits()
                    } else if moving {
                        1.0_f64.to_bits()
                    } else {
                        0.0_f64.to_bits()
                    },
                    chebyshev_coefficients: if *edge == target {
                        target_coefficients.clone()
                    } else if moving {
                        vec![zero, slope]
                    } else {
                        vec![zero]
                    },
                }
            })
            .collect(),
        CycleScheduleLimitsV1 {
            max_degree,
            ..fixture.schedule_limits
        },
    )
    .unwrap()
}

#[test]
fn accepts_multiple_stationary_components_parallel_edges_and_effective_orientation() {
    let fixture = oriented_generalized_cut_fixture();
    assert!(matches!(
        recognize(
            &fixture,
            &fixture.geometry,
            &fixture.schedule,
            &fixture.canonical_hinge_indices,
            &fixture.canonical_edges,
        )
        .unwrap(),
        ExactParallelCutRecognitionV2::Proven { .. }
    ));
    let material = fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            0.0,
            generous_limits(fixture.schedule_limits),
            || Ok(()),
        )
        .unwrap();
    assert_eq!(material.partition(), &[(0, 0)]);
    assert_eq!(material.resources().visited_partition_nodes, 1);
}

#[test]
fn accepts_every_cardinal_axis_and_canonicalizes_signed_zero() {
    let fixture = exact_parallel_cut_fixture();
    for (start, end, axis) in [
        (
            Point3::new(0.0, -0.0, 0.0).unwrap(),
            Point3::new(0.0, 1.0, 0.0).unwrap(),
            Point3::new(-0.0, 1.0, 0.0).unwrap(),
        ),
        (
            Point3::new(0.0, 0.0, -0.0).unwrap(),
            Point3::new(0.0, 0.0, 1.0).unwrap(),
            Point3::new(0.0, -0.0, 1.0).unwrap(),
        ),
    ] {
        let geometry = MaterialHingeGraphGeometry::new_for_test(
            fixture.geometry.face_ids().to_vec(),
            fixture
                .geometry
                .hinges()
                .iter()
                .map(|hinge| {
                    if fixture.moving_edges.contains(&hinge.edge()) {
                        TreeHinge::new_for_test(
                            hinge.edge(),
                            hinge.assignment(),
                            hinge.left_face(),
                            hinge.right_face(),
                            start,
                            end,
                            axis,
                        )
                    } else {
                        hinge.clone()
                    }
                })
                .collect(),
        );
        assert!(matches!(
            recognize(
                &fixture,
                &geometry,
                &fixture.schedule,
                &fixture.canonical_hinge_indices,
                &fixture.canonical_edges,
            )
            .unwrap(),
            ExactParallelCutRecognitionV2::Proven { .. }
        ));
    }
}

#[test]
fn rejects_moving_self_loop_nonunit_axis_duplicate_and_noncanonical_carriers() {
    let fixture = exact_parallel_cut_fixture();
    let target = fixture.moving_edges[0];
    let self_loop = MaterialHingeGraphGeometry::new_for_test(
        fixture.geometry.face_ids().to_vec(),
        fixture
            .geometry
            .hinges()
            .iter()
            .map(|hinge| {
                if hinge.edge() == target {
                    TreeHinge::new_for_test(
                        hinge.edge(),
                        hinge.assignment(),
                        hinge.left_face(),
                        hinge.left_face(),
                        hinge.start(),
                        hinge.end(),
                        hinge.axis(),
                    )
                } else {
                    hinge.clone()
                }
            })
            .collect(),
    );
    let nonunit = exact_parallel_cut_geometry_with_mutation(
        &fixture,
        target,
        FoldAssignment::Mountain,
        Point3::new(0.0, 0.0, 0.0).unwrap(),
        Point3::new(1.0, 0.0, 0.0).unwrap(),
        Point3::new(2.0, 0.0, 0.0).unwrap(),
    );
    assert!(Point3::new(f64::NAN, 0.0, 0.0).is_err());
    for geometry in [&self_loop, &nonunit] {
        assert!(matches!(
            recognize(
                &fixture,
                geometry,
                &fixture.schedule,
                &fixture.canonical_hinge_indices,
                &fixture.canonical_edges,
            )
            .unwrap(),
            ExactParallelCutRecognitionV2::NotApplicable { .. }
        ));
    }

    let mut duplicate_indices = fixture.canonical_hinge_indices.clone();
    let mut duplicate_edges = fixture.canonical_edges.clone();
    duplicate_indices[1] = duplicate_indices[0];
    duplicate_edges[1] = duplicate_edges[0];
    let mut checkpoint = || Ok(());
    assert!(
        !validate_carrier_with_checkpoint_v2(
            &fixture.geometry,
            &fixture.audit,
            &duplicate_indices,
            &duplicate_edges,
            &mut checkpoint,
        )
        .unwrap()
    );

    let mut reversed_indices = fixture.canonical_hinge_indices.clone();
    let mut reversed_edges = fixture.canonical_edges.clone();
    reversed_indices.reverse();
    reversed_edges.reverse();
    assert!(
        !validate_carrier_with_checkpoint_v2(
            &fixture.geometry,
            &fixture.audit,
            &reversed_indices,
            &reversed_edges,
            &mut checkpoint,
        )
        .unwrap()
    );
}

#[test]
fn rejects_profile_bit_difference_higher_degree_and_closed_range_boundaries() {
    let fixture = exact_parallel_cut_fixture();
    let target = fixture.moving_edges[0];
    let zero = RationalCoefficientV1 {
        numerator: 0,
        denominator: 1,
    };
    let half = RationalCoefficientV1 {
        numerator: 1,
        denominator: 2,
    };
    let quarter = RationalCoefficientV1 {
        numerator: 1,
        denominator: 4,
    };
    let schedules = [
        schedule_with_profile_override(
            &fixture,
            target,
            f64::from_bits(1.0_f64.to_bits() + 1),
            vec![zero, half],
            1,
        ),
        schedule_with_profile_override(&fixture, target, 1.0, vec![zero, half, quarter], 2),
        schedule_with_profile_override(&fixture, target, 0.5, vec![zero, half], 1),
        schedule_with_profile_override(&fixture, target, 179.5, vec![zero, half], 1),
    ];
    for schedule in &schedules {
        assert!(matches!(
            recognize(
                &fixture,
                &fixture.geometry,
                schedule,
                &fixture.canonical_hinge_indices,
                &fixture.canonical_edges,
            )
            .unwrap(),
            ExactParallelCutRecognitionV2::NotApplicable { .. }
        ));
    }
}
