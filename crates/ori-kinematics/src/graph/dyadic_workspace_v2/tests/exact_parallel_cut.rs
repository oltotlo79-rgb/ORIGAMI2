use super::*;

mod binding;
mod geometry_and_profile;
mod resources_and_stop;

struct ExactParallelCutFixture {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    fixed_face: FaceId,
    canonical_hinge_indices: Vec<usize>,
    canonical_edges: Vec<EdgeId>,
    moving_edges: Vec<EdgeId>,
    internal_edges: Vec<EdgeId>,
    schedule: CanonicalCycleScheduleV1,
    schedule_limits: CycleScheduleLimitsV1,
}

fn exact_parallel_cut_fixture() -> ExactParallelCutFixture {
    let namespace = ProjectId::schema_namespace([
        0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e, 0x8f,
        0x90,
    ]);
    let faces = (0..6)
        .map(|index| FaceId::derive_v5(namespace, format!("cut-face-{index}").as_bytes()))
        .collect::<Vec<_>>();
    let edges = (0..7)
        .map(|index| EdgeId::derive_v5(namespace, format!("cut-edge-{index}").as_bytes()))
        .collect::<Vec<_>>();
    let triples = vec![
        (edges[0], faces[0], faces[1]),
        (edges[1], faces[1], faces[2]),
        (edges[2], faces[3], faces[4]),
        (edges[3], faces[4], faces[5]),
        (edges[4], faces[0], faces[3]),
        (edges[5], faces[1], faces[4]),
        (edges[6], faces[2], faces[5]),
    ];
    let audit = MaterialHingeGraphAudit::prepare(
        &topology(&faces, &triples),
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
    let axis = Point3::new(1.0, 0.0, 0.0).unwrap();
    let geometry = MaterialHingeGraphGeometry::new_for_test(
        audit.faces().to_vec(),
        triples
            .iter()
            .map(|(edge, left, right)| {
                TreeHinge::new_for_test(
                    *edge,
                    FoldAssignment::Mountain,
                    *left,
                    *right,
                    origin,
                    axis,
                    axis,
                )
            })
            .collect(),
    );
    let moving_edges = edges[4..].to_vec();
    let internal_edges = edges[..4].to_vec();
    let schedule_limits = CycleScheduleLimitsV1 {
        max_hinges: edges.len(),
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
    let mut canonical_edges = edges.clone();
    canonical_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
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
                    1.0_f64.to_bits()
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
    let mut canonical_hinge_indices = (0..geometry.hinges().len()).collect::<Vec<_>>();
    canonical_hinge_indices
        .sort_unstable_by_key(|index| geometry.hinges()[*index].edge().canonical_bytes());
    ExactParallelCutFixture {
        geometry,
        audit,
        fixed_face,
        canonical_hinge_indices,
        canonical_edges,
        moving_edges,
        internal_edges,
        schedule,
        schedule_limits,
    }
}

fn exact_parallel_cut_schedule_with_overrides(
    fixture: &ExactParallelCutFixture,
    moving_edges: &[EdgeId],
    nonzero_constant_edge: Option<EdgeId>,
) -> CanonicalCycleScheduleV1 {
    exact_parallel_cut_schedule_for_geometry_with_overrides(
        fixture,
        &fixture.geometry,
        moving_edges,
        nonzero_constant_edge,
    )
}

fn exact_parallel_cut_schedule_for_geometry_with_overrides(
    fixture: &ExactParallelCutFixture,
    geometry: &MaterialHingeGraphGeometry,
    moving_edges: &[EdgeId],
    nonzero_constant_edge: Option<EdgeId>,
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
        geometry,
        &fixture.audit,
        fixture.fixed_face,
        [-1.0, 1.0],
        fixture
            .canonical_edges
            .iter()
            .map(|edge| {
                let moving = moving_edges.contains(edge);
                CycleScheduleEntryInputV1 {
                    edge: *edge,
                    initial_angle_degrees_bits: if moving
                        || nonzero_constant_edge.is_some_and(|candidate| candidate == *edge)
                    {
                        1.0_f64.to_bits()
                    } else {
                        0.0_f64.to_bits()
                    },
                    chebyshev_coefficients: if moving {
                        vec![zero, slope]
                    } else {
                        vec![zero]
                    },
                }
            })
            .collect(),
        fixture.schedule_limits,
    )
    .unwrap()
}

fn exact_parallel_cut_geometry_with_mutation(
    fixture: &ExactParallelCutFixture,
    target: EdgeId,
    assignment: FoldAssignment,
    start: Point3,
    end: Point3,
    axis: Point3,
) -> MaterialHingeGraphGeometry {
    MaterialHingeGraphGeometry::new_for_test(
        fixture.geometry.face_ids().to_vec(),
        fixture
            .geometry
            .hinges()
            .iter()
            .map(|hinge| {
                if hinge.edge() == target {
                    TreeHinge::new_for_test(
                        hinge.edge(),
                        assignment,
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
    )
}

#[test]
fn exact_parallel_cut_theorem_is_one_leaf_and_has_tight_dedicated_resources() {
    let fixture = exact_parallel_cut_fixture();
    let generous = generous_limits(fixture.schedule_limits);
    let first = fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            0.0,
            generous,
            || Ok(()),
        )
        .unwrap();
    let resources = first.resources();
    assert_eq!(first.partition(), &[(0, 0)]);
    assert!(resources.charged_theorem_recognizer_work > 0);
    assert!(resources.charged_theorem_recognizer_upper_bound_bytes > 0);
    assert_eq!(resources.visited_partition_nodes, 1);
    assert_eq!(resources.issued_leaves, 1);
    assert_eq!(
        resources.charged_carrier_index_workspace_upper_bound_bytes,
        std::mem::size_of::<usize>() * fixture.geometry.hinges().len(),
        "the theorem scratch must not be folded into the carrier subceiling"
    );

    let exact = exact_limits(generous, resources);
    let exact_issue = fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.schedule,
            0.0,
            exact,
            || Ok(()),
        )
        .unwrap();
    assert_eq!(exact_issue.resources(), resources);

    for one_short in [
        DyadicIntervalClosureWorkspaceLimitsV2 {
            max_theorem_recognizer_work: resources.charged_theorem_recognizer_work - 1,
            ..exact
        },
        DyadicIntervalClosureWorkspaceLimitsV2 {
            max_theorem_recognizer_workspace_bytes: resources
                .charged_theorem_recognizer_upper_bound_bytes
                - 1,
            ..exact
        },
    ] {
        assert!(matches!(
            fixture
                .geometry
                .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
                    &fixture.audit,
                    fixture.fixed_face,
                    &fixture.schedule,
                    0.0,
                    one_short,
                    || Ok(()),
                ),
            Err(DyadicIntervalClosureControlErrorV1::Closure(
                DyadicIntervalClosureErrorV1::ResourceLimit
            ))
        ));
    }
}

#[test]
fn exact_parallel_cut_theorem_rejects_offset_axis_sign_and_noncut_profiles() {
    let fixture = exact_parallel_cut_fixture();
    let recognize = |geometry: &MaterialHingeGraphGeometry,
                     schedule: &CanonicalCycleScheduleV1,
                     canonical_edges: &[EdgeId]| {
        recognize_exact_parallel_cut_with_checkpoint_v2(
            geometry,
            schedule,
            &fixture.canonical_hinge_indices,
            canonical_edges,
            1_000_000,
            1_000_000,
            &mut || Ok(()),
        )
    };
    assert!(matches!(
        recognize(
            &fixture.geometry,
            &fixture.schedule,
            &fixture.canonical_edges
        )
        .unwrap(),
        ExactParallelCutRecognitionV2::Proven { .. }
    ));

    let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
    let x = Point3::new(1.0, 0.0, 0.0).unwrap();
    let offset_start = Point3::new(0.0, 1.0, 0.0).unwrap();
    let offset_end = Point3::new(1.0, 1.0, 0.0).unwrap();
    let diagonal_axis = Point3::new(1.0, 1.0, 0.0).unwrap();
    let target = fixture.moving_edges[1];
    let offset = exact_parallel_cut_geometry_with_mutation(
        &fixture,
        target,
        FoldAssignment::Mountain,
        offset_start,
        offset_end,
        x,
    );
    let noncardinal = exact_parallel_cut_geometry_with_mutation(
        &fixture,
        target,
        FoldAssignment::Mountain,
        origin,
        x,
        diagonal_axis,
    );
    let wrong_sign = exact_parallel_cut_geometry_with_mutation(
        &fixture,
        target,
        FoldAssignment::Valley,
        origin,
        x,
        x,
    );
    for geometry in [&offset, &noncardinal, &wrong_sign] {
        assert!(matches!(
            recognize(geometry, &fixture.schedule, &fixture.canonical_edges).unwrap(),
            ExactParallelCutRecognitionV2::NotApplicable { .. }
        ));
    }

    let mut extra_moving = fixture.moving_edges.clone();
    extra_moving.push(fixture.internal_edges[0]);
    let noncut = exact_parallel_cut_schedule_with_overrides(&fixture, &extra_moving, None);
    assert!(matches!(
        recognize(&fixture.geometry, &noncut, &fixture.canonical_edges).unwrap(),
        ExactParallelCutRecognitionV2::NotApplicable { .. }
    ));

    let nonzero_constant = exact_parallel_cut_schedule_with_overrides(
        &fixture,
        &fixture.moving_edges,
        Some(fixture.internal_edges[0]),
    );
    assert!(matches!(
        recognize(
            &fixture.geometry,
            &nonzero_constant,
            &fixture.canonical_edges
        )
        .unwrap(),
        ExactParallelCutRecognitionV2::NotApplicable {
            workspace_bytes: 0,
            ..
        }
    ));

    let mut foreign_edges = fixture.canonical_edges.clone();
    foreign_edges[0] = EdgeId::derive_v5(
        ProjectId::schema_namespace([0xa5; 16]),
        b"foreign-theorem-carrier",
    );
    assert!(matches!(
        recognize(&fixture.geometry, &fixture.schedule, &foreign_edges).unwrap(),
        ExactParallelCutRecognitionV2::NotApplicable {
            workspace_bytes: 0,
            ..
        }
    ));
}
