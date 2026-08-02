use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use super::*;
use crate::{
    CycleScheduleEntryInputV1, HalfAngleRationalEntryInputV1, Point3, RationalCoefficientV1,
};

fn face(id: FaceId) -> Face {
    Face {
        id,
        key: FaceKey(id.canonical_bytes().repeat(2).try_into().unwrap()),
        outer: BoundaryWalk {
            half_edges: Vec::new(),
            signed_double_area: 1.0,
        },
        holes: Vec::new(),
        seams: Vec::new(),
        area: 0.5,
    }
}

fn topology(faces: &[FaceId], hinges: &[(EdgeId, FaceId, FaceId)]) -> TopologySnapshot {
    TopologySnapshot {
        source_revision: 1,
        faces: faces.iter().copied().map(face).collect(),
        edge_incidence: Vec::new(),
        hinge_adjacency: hinges
            .iter()
            .map(|(edge, first, second)| FaceAdjacency {
                edge: *edge,
                first: *first,
                second: *second,
                assignment: FoldAssignment::Mountain,
            })
            .collect(),
        material_components: Vec::new(),
    }
}

struct Fixture {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    fixed_face: FaceId,
    ordinary: CanonicalCycleScheduleV1,
    exact: CanonicalCycleScheduleV1,
    schedule_limits: CycleScheduleLimitsV1,
}

fn fixture() -> Fixture {
    let namespace = ProjectId::schema_namespace([
        0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f,
        0x40,
    ]);
    let faces = [b"workspace-a", b"workspace-b", b"workspace-c"]
        .map(|name| FaceId::derive_v5(namespace, name));
    let edges = [b"workspace-ab", b"workspace-bc", b"workspace-ca"]
        .map(|name| EdgeId::derive_v5(namespace, name));
    let topology = topology(
        &faces,
        &[
            (edges[0], faces[0], faces[1]),
            (edges[1], faces[1], faces[2]),
            (edges[2], faces[2], faces[0]),
        ],
    );
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
    let axis = Point3::new(1.0, 0.0, 0.0).unwrap();
    let geometry = MaterialHingeGraphGeometry::new_for_test(
        audit.faces().to_vec(),
        [
            (edges[2], faces[2], faces[0]),
            (edges[0], faces[0], faces[1]),
            (edges[1], faces[1], faces[2]),
        ]
        .into_iter()
        .map(|(edge, left, right)| {
            TreeHinge::new_for_test(
                edge,
                FoldAssignment::Mountain,
                left,
                right,
                origin,
                axis,
                axis,
            )
        })
        .collect(),
    );
    let fixed_face = audit.faces()[0];
    let schedule_limits = CycleScheduleLimitsV1 {
        max_hinges: 3,
        max_degree: 0,
        max_coefficient_bits: 8,
        max_work: 1_024,
    };
    let mut canonical_edges = edges.to_vec();
    canonical_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let zero = RationalCoefficientV1 {
        numerator: 0,
        denominator: 1,
    };
    let one = RationalCoefficientV1 {
        numerator: 1,
        denominator: 1,
    };
    let ordinary = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed_face,
        [0.0, 1.0],
        canonical_edges
            .iter()
            .map(|edge| CycleScheduleEntryInputV1 {
                edge: *edge,
                initial_angle_degrees_bits: 120.0_f64.to_bits(),
                chebyshev_coefficients: vec![zero],
            })
            .collect(),
        schedule_limits,
    )
    .unwrap();
    let exact = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed_face,
        canonical_edges
            .iter()
            .map(|edge| HalfAngleRationalEntryInputV1 {
                edge: *edge,
                u_domain: [zero, one],
                numerator_power_coefficients: vec![zero],
                denominator_power_coefficients: vec![one],
            })
            .collect(),
        schedule_limits,
    )
    .unwrap();
    Fixture {
        geometry,
        audit,
        fixed_face,
        ordinary,
        exact,
        schedule_limits,
    }
}

fn nonstationary_exact_tree_fixture() -> Fixture {
    let namespace = ProjectId::schema_namespace([
        0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f,
        0x50,
    ]);
    let faces = [b"exact-tree-a", b"exact-tree-b"].map(|name| FaceId::derive_v5(namespace, name));
    let edge = EdgeId::derive_v5(namespace, b"exact-tree-edge");
    let topology = topology(&faces, &[(edge, faces[0], faces[1])]);
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
    let axis = Point3::new(1.0, 0.0, 0.0).unwrap();
    let geometry = MaterialHingeGraphGeometry::new_for_test(
        audit.faces().to_vec(),
        vec![TreeHinge::new_for_test(
            edge,
            FoldAssignment::Mountain,
            faces[0],
            faces[1],
            origin,
            axis,
            axis,
        )],
    );
    let fixed_face = audit.faces()[0];
    let schedule_limits = CycleScheduleLimitsV1 {
        max_hinges: 1,
        max_degree: 1,
        max_coefficient_bits: 64,
        max_work: 4_096,
    };
    let rational = |numerator, denominator| RationalCoefficientV1 {
        numerator,
        denominator,
    };
    let ordinary = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        fixed_face,
        [0.0, 1.0],
        vec![CycleScheduleEntryInputV1 {
            edge,
            initial_angle_degrees_bits: 90.0_f64.to_bits(),
            chebyshev_coefficients: vec![rational(0, 1), rational(1, 1)],
        }],
        schedule_limits,
    )
    .unwrap();
    let exact = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        &geometry,
        &audit,
        fixed_face,
        vec![HalfAngleRationalEntryInputV1 {
            edge,
            u_domain: [rational(0, 1), rational(1, 1)],
            numerator_power_coefficients: vec![rational(1, 1), rational(1, 1)],
            denominator_power_coefficients: vec![rational(1, 1)],
        }],
        schedule_limits,
    )
    .unwrap();
    Fixture {
        geometry,
        audit,
        fixed_face,
        ordinary,
        exact,
        schedule_limits,
    }
}

fn adaptive_correlated_cycle_fixture() -> Fixture {
    let mut fixture = fixture();
    let schedule_limits = CycleScheduleLimitsV1 {
        max_hinges: fixture.geometry.hinges().len(),
        max_degree: 1,
        max_coefficient_bits: 53,
        max_work: 4_096,
    };
    let zero = RationalCoefficientV1 {
        numerator: 0,
        denominator: 1,
    };
    let mut edges = fixture
        .geometry
        .hinges()
        .iter()
        .map(TreeHinge::edge)
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let ordinary = CanonicalCycleScheduleV1::prepare(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        [0.0, 1.0],
        edges
            .iter()
            .enumerate()
            .map(|(index, edge)| CycleScheduleEntryInputV1 {
                edge: *edge,
                initial_angle_degrees_bits: 120.0_f64.to_bits(),
                chebyshev_coefficients: vec![
                    zero,
                    RationalCoefficientV1 {
                        numerator: if index < 2 { 1 } else { -2 },
                        denominator: 1,
                    },
                ],
            })
            .collect(),
        schedule_limits,
    )
    .unwrap();
    fixture.ordinary = ordinary;
    fixture.schedule_limits = schedule_limits;
    fixture
}

fn generous_limits(
    schedule_limits: CycleScheduleLimitsV1,
) -> DyadicIntervalClosureWorkspaceLimitsV2 {
    let ceiling = usize::MAX - 1;
    DyadicIntervalClosureWorkspaceLimitsV2 {
        max_depth: 0,
        max_leaves: 1,
        max_work: 1_000_000,
        schedule_limits,
        max_carrier_index_workspace_bytes: ceiling,
        max_schedule_evaluation_workspace_bytes: ceiling,
        max_big_rational_payload_bytes: ceiling,
        max_exact_rational_object_bytes: ceiling,
        max_interval_closure_workspace_bytes: ceiling,
        max_partition_workspace_bytes: ceiling,
        max_retained_material_bytes: ceiling,
        max_publication_workspace_bytes: ceiling,
        max_peak_workspace_bytes: ceiling,
    }
}

fn exact_limits(
    mut limits: DyadicIntervalClosureWorkspaceLimitsV2,
    resources: DyadicIntervalClosureWorkspaceResourcesV2,
) -> DyadicIntervalClosureWorkspaceLimitsV2 {
    limits.max_carrier_index_workspace_bytes =
        resources.charged_carrier_index_workspace_upper_bound_bytes;
    limits.max_schedule_evaluation_workspace_bytes =
        resources.charged_schedule_evaluation_workspace_upper_bound_bytes;
    limits.max_big_rational_payload_bytes =
        resources.charged_big_rational_payload_upper_bound_bytes;
    limits.max_exact_rational_object_bytes =
        resources.charged_exact_rational_object_upper_bound_bytes;
    limits.max_interval_closure_workspace_bytes =
        resources.charged_interval_closure_workspace_upper_bound_bytes;
    limits.max_partition_workspace_bytes = resources.charged_partition_workspace_upper_bound_bytes;
    limits.max_retained_material_bytes = resources.charged_retained_material_upper_bound_bytes;
    limits.max_publication_workspace_bytes =
        resources.charged_publication_workspace_upper_bound_bytes;
    limits.max_peak_workspace_bytes = resources.charged_peak_workspace_upper_bound_bytes;
    limits
}

fn issue(
    fixture: &Fixture,
    schedule: &CanonicalCycleScheduleV1,
    limits: DyadicIntervalClosureWorkspaceLimitsV2,
) -> Result<WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2, DyadicIntervalClosureControlErrorV1>
{
    issue_at_tolerance(fixture, schedule, limits, 1.0e-8)
}

fn issue_at_tolerance(
    fixture: &Fixture,
    schedule: &CanonicalCycleScheduleV1,
    limits: DyadicIntervalClosureWorkspaceLimitsV2,
    tolerance: f64,
) -> Result<WorkspaceBoundedDyadicMaterialHingeIntervalClosureV2, DyadicIntervalClosureControlErrorV1>
{
    fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            schedule,
            tolerance,
            limits,
            || Ok(()),
        )
}

#[test]
fn exact_half_angle_workspace_is_tight_and_every_byte_one_short_fails() {
    let fixture = nonstationary_exact_tree_fixture();
    let generous = generous_limits(fixture.schedule_limits);
    assert!(fixture.geometry.hinges().iter().all(|hinge| {
        fixture
            .exact
            .derivative_bound(hinge.edge())
            .is_some_and(|bound| bound > 0.0)
    }));
    let bound = fixture
        .exact
        .checked_dyadic_workspace_upper_bound_v2(0, fixture.schedule_limits)
        .unwrap();
    let legacy_boxes = fixture
        .exact
        .evaluate_angle_box_dyadic(0, 0, fixture.schedule_limits)
        .unwrap();
    let metered_evaluation = fixture
        .exact
        .evaluate_angle_box_dyadic_with_workspace_v2(
            0,
            0,
            fixture.schedule_limits,
            bound,
            usize::MAX - 1,
        )
        .unwrap();
    assert_eq!(metered_evaluation.angle_boxes, legacy_boxes);
    assert!(metered_evaluation.exact_vector_capacity_peak_bytes > 0);
    let first = issue(&fixture, &fixture.exact, generous).unwrap();
    let resources = first.resources();
    assert!(resources.charged_big_rational_payload_upper_bound_bytes > 0);
    assert_eq!(resources.charged_theorem_recognizer_upper_bound_bytes, 0);
    assert_eq!(first.partition(), &[(0, 0)]);
    assert_eq!(
        first.canonical_checked_hinges().len(),
        fixture.geometry.hinges().len()
    );
    assert!(first.has_nonempty_canonical_complete_partition_v2());
    assert!(first.issuer_geometry.matches(&fixture.geometry));
    assert_eq!(first.fixed_face, fixture.fixed_face);
    assert_eq!(first.tolerance_bits, 1.0e-8_f64.to_bits());
    assert_eq!(first.policy, generous);
    assert_eq!(
        first.schedule_binding_fingerprint_v2,
        fixture.exact.certificate_binding_fingerprint_v2()
    );
    assert_eq!(
        first.graph_binding_fingerprint_v1,
        fixture.exact.graph_binding_fingerprint_v1()
    );
    let exact = exact_limits(generous, resources);
    let second = issue(&fixture, &fixture.exact, exact).unwrap();
    assert_eq!(second.resources(), resources);
    assert_ne!(
        second.partition_binding_fingerprint_v2(),
        first.partition_binding_fingerprint_v2()
    );
    let mut exact_object_policy_mutation = generous;
    exact_object_policy_mutation.max_exact_rational_object_bytes -= 1;
    let policy_mutated = issue(&fixture, &fixture.exact, exact_object_policy_mutation).unwrap();
    assert_ne!(
        policy_mutated.partition_binding_fingerprint_v2(),
        first.partition_binding_fingerprint_v2()
    );

    let mut cases = Vec::new();
    macro_rules! one_short {
        ($field:ident, $resource:ident) => {{
            assert!(resources.$resource > 0);
            let mut candidate = exact;
            candidate.$field = resources.$resource - 1;
            cases.push(candidate);
        }};
    }
    one_short!(
        max_carrier_index_workspace_bytes,
        charged_carrier_index_workspace_upper_bound_bytes
    );
    one_short!(
        max_schedule_evaluation_workspace_bytes,
        charged_schedule_evaluation_workspace_upper_bound_bytes
    );
    one_short!(
        max_big_rational_payload_bytes,
        charged_big_rational_payload_upper_bound_bytes
    );
    one_short!(
        max_exact_rational_object_bytes,
        charged_exact_rational_object_upper_bound_bytes
    );
    one_short!(
        max_interval_closure_workspace_bytes,
        charged_interval_closure_workspace_upper_bound_bytes
    );
    one_short!(
        max_partition_workspace_bytes,
        charged_partition_workspace_upper_bound_bytes
    );
    one_short!(
        max_retained_material_bytes,
        charged_retained_material_upper_bound_bytes
    );
    one_short!(
        max_publication_workspace_bytes,
        charged_publication_workspace_upper_bound_bytes
    );
    one_short!(
        max_peak_workspace_bytes,
        charged_peak_workspace_upper_bound_bytes
    );
    for candidate in cases {
        assert!(matches!(
            issue(&fixture, &fixture.exact, candidate),
            Err(DyadicIntervalClosureControlErrorV1::Closure(
                DyadicIntervalClosureErrorV1::ResourceLimit
            ))
        ));
    }

    let mut hinge_short = generous;
    hinge_short.schedule_limits.max_hinges = 0;
    let mut degree_short = generous;
    degree_short.schedule_limits.max_degree = 0;
    let mut bits_exact = generous;
    bits_exact.schedule_limits.max_coefficient_bits = 2;
    assert!(issue(&fixture, &fixture.exact, bits_exact).is_ok());
    let mut bits_short = generous;
    bits_short.schedule_limits.max_coefficient_bits = 1;
    let mut work_exact = generous;
    work_exact.schedule_limits.max_work = 297;
    assert!(issue(&fixture, &fixture.exact, work_exact).is_ok());
    let mut work_short = generous;
    work_short.schedule_limits.max_work = 296;
    for candidate in [hinge_short, degree_short, bits_short, work_short] {
        assert!(matches!(
            issue(&fixture, &fixture.exact, candidate),
            Err(DyadicIntervalClosureControlErrorV1::Closure(
                DyadicIntervalClosureErrorV1::ResourceLimit
            ))
        ));
    }

    let legacy = fixture
        .geometry
        .prove_dyadic_schedule_closure_v1(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.exact,
            1.0e-8,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 0,
                max_leaves: 1,
                max_work: 1_000_000,
                schedule_limits: fixture.schedule_limits,
            },
        )
        .unwrap();
    assert_eq!(legacy.leaves().len(), first.partition().len());
}

#[test]
fn every_usize_max_limit_is_rejected_as_resource_limit() {
    let fixture = fixture();
    let base = generous_limits(fixture.schedule_limits);
    let mut cases = Vec::new();
    macro_rules! maximum {
        ($field:ident) => {{
            let mut candidate = base;
            candidate.$field = usize::MAX;
            cases.push(candidate);
        }};
    }
    maximum!(max_leaves);
    maximum!(max_work);
    maximum!(max_carrier_index_workspace_bytes);
    maximum!(max_schedule_evaluation_workspace_bytes);
    maximum!(max_big_rational_payload_bytes);
    maximum!(max_exact_rational_object_bytes);
    maximum!(max_interval_closure_workspace_bytes);
    maximum!(max_partition_workspace_bytes);
    maximum!(max_retained_material_bytes);
    maximum!(max_publication_workspace_bytes);
    maximum!(max_peak_workspace_bytes);
    for field in 0..3 {
        let mut candidate = base;
        match field {
            0 => candidate.schedule_limits.max_hinges = usize::MAX,
            1 => candidate.schedule_limits.max_degree = usize::MAX,
            _ => candidate.schedule_limits.max_work = usize::MAX,
        }
        cases.push(candidate);
    }
    let mut max_coefficient_bits = base;
    max_coefficient_bits.schedule_limits.max_coefficient_bits = u32::MAX;
    cases.push(max_coefficient_bits);
    for candidate in cases {
        assert!(matches!(
            issue(&fixture, &fixture.exact, candidate),
            Err(DyadicIntervalClosureControlErrorV1::Closure(
                DyadicIntervalClosureErrorV1::ResourceLimit
            ))
        ));
    }
    for depth in [64, u32::MAX] {
        let mut candidate = base;
        candidate.max_depth = depth;
        assert!(matches!(
            issue(&fixture, &fixture.exact, candidate),
            Err(DyadicIntervalClosureControlErrorV1::Closure(
                DyadicIntervalClosureErrorV1::InvalidInput
            ))
        ));
    }
}

#[test]
fn foreign_audit_carrier_and_checked_arithmetic_overflow_fail_closed() {
    let fixture = fixture();
    let limits = generous_limits(fixture.schedule_limits);
    let mut foreign_audit = fixture.audit.clone();
    foreign_audit.closure_hinges[0] = EdgeId::derive_v5(
        ProjectId::schema_namespace([
            0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x7b, 0x7c, 0x7d, 0x7e,
            0x7f, 0x80,
        ]),
        b"foreign-audit-edge",
    );
    let mut canonical_indices = (0..fixture.geometry.hinges().len()).collect::<Vec<_>>();
    canonical_indices
        .sort_unstable_by_key(|index| fixture.geometry.hinges()[*index].edge().canonical_bytes());
    let canonical_edges = canonical_indices
        .iter()
        .map(|index| fixture.geometry.hinges()[*index].edge())
        .collect::<Vec<_>>();
    let mut checkpoint = || -> Result<(), DyadicIntervalClosureStopV1> { Ok(()) };
    assert!(
        !validate_carrier_with_checkpoint_v2(
            &fixture.geometry,
            &foreign_audit,
            &canonical_indices,
            &canonical_edges,
            &mut checkpoint,
        )
        .unwrap()
    );
    let foreign = fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &foreign_audit,
            fixture.fixed_face,
            &fixture.exact,
            1.0e-8,
            limits,
            || Ok(()),
        );
    assert!(matches!(
        foreign,
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::InvalidInput
        ))
    ));

    let mut partition_overflow = limits;
    partition_overflow.max_leaves = usize::MAX - 1;
    assert!(matches!(
        issue(&fixture, &fixture.exact, partition_overflow),
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::ResourceLimit
        ))
    ));
    let mut exact_overflow = limits;
    exact_overflow.schedule_limits.max_work = usize::MAX - 1;
    assert!(matches!(
        issue(&fixture, &fixture.exact, exact_overflow),
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::ResourceLimit
        ))
    ));
}

#[test]
fn stop_class_is_exact_at_entry_and_publication() {
    let fixture = fixture();
    let limits = generous_limits(fixture.schedule_limits);
    let mut polls = 0usize;
    fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.exact,
            1.0e-8,
            limits,
            || {
                polls += 1;
                Ok(())
            },
        )
        .unwrap();
    let successful_poll_count = polls;
    assert!(successful_poll_count > 1);
    for stop in [
        DyadicIntervalClosureStopV1::Cancelled,
        DyadicIntervalClosureStopV1::DeadlineExceeded,
    ] {
        for stop_at in 1..=successful_poll_count {
            let mut polls = 0usize;
            let result = fixture
                .geometry
                .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
                    &fixture.audit,
                    fixture.fixed_face,
                    &fixture.exact,
                    1.0e-8,
                    limits,
                    || {
                        polls += 1;
                        if polls == stop_at { Err(stop) } else { Ok(()) }
                    },
                );
            match stop {
                DyadicIntervalClosureStopV1::Cancelled => assert!(matches!(
                    result,
                    Err(DyadicIntervalClosureControlErrorV1::Cancelled)
                )),
                DyadicIntervalClosureStopV1::DeadlineExceeded => assert!(matches!(
                    result,
                    Err(DyadicIntervalClosureControlErrorV1::DeadlineExceeded)
                )),
            }
        }
    }
}

#[test]
fn adaptive_split_has_tight_depth_leaf_work_and_split_stop_boundaries() {
    let fixture = adaptive_correlated_cycle_fixture();
    let tolerance = 0.1;
    let mut limits = generous_limits(fixture.schedule_limits);
    limits.max_depth = 2;
    limits.max_leaves = 4;
    limits.max_work = 7_202;
    let material = issue_at_tolerance(&fixture, &fixture.ordinary, limits, tolerance)
        .expect("the fixed correlated schedule closes on four depth-two leaves");
    assert!(fixture.geometry.hinges().iter().all(|hinge| {
        fixture
            .ordinary
            .derivative_bound(hinge.edge())
            .is_some_and(|bound| bound > 0.0)
    }));
    assert!(material.has_nonempty_canonical_complete_partition_v2());
    assert_eq!(material.partition(), &[(2, 0), (2, 1), (2, 2), (2, 3)]);
    let resources = material.resources();
    assert_eq!(resources.charged_theorem_recognizer_upper_bound_bytes, 0);
    assert_eq!(resources.issued_leaves, 4);
    assert_eq!(resources.visited_partition_nodes, 7);
    let legacy = fixture
        .geometry
        .prove_dyadic_schedule_closure_v1(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.ordinary,
            tolerance,
            DyadicIntervalClosureLimitsV1 {
                max_depth: limits.max_depth,
                max_leaves: limits.max_leaves,
                max_work: limits.max_work,
                schedule_limits: limits.schedule_limits,
            },
        )
        .unwrap();
    assert_eq!(
        legacy
            .leaves()
            .iter()
            .map(|(depth, index, _)| (*depth, *index))
            .collect::<Vec<_>>(),
        material.partition()
    );

    let mut depth_short = limits;
    depth_short.max_depth = 1;
    assert!(matches!(
        issue_at_tolerance(&fixture, &fixture.ordinary, depth_short, tolerance),
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::UnprovenClosure { .. }
        ))
    ));
    let mut leaves_short = limits;
    leaves_short.max_leaves = 3;
    assert!(matches!(
        issue_at_tolerance(&fixture, &fixture.ordinary, leaves_short, tolerance),
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::ResourceLimit
        ))
    ));
    let mut work_short = limits;
    work_short.max_work = 7_201;
    assert!(matches!(
        issue_at_tolerance(&fixture, &fixture.ordinary, work_short, tolerance),
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::ResourceLimit
        ))
    ));

    let mut depth_zero = limits;
    depth_zero.max_depth = 0;
    depth_zero.max_leaves = 1;
    let mut root_polls = 0usize;
    let root = fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.ordinary,
            tolerance,
            depth_zero,
            || {
                root_polls += 1;
                Ok(())
            },
        );
    assert!(matches!(
        root,
        Err(DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::UnprovenClosure { .. }
        ))
    ));
    let split_poll = root_polls + 1;
    let mut polls = 0usize;
    let stopped = fixture
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.ordinary,
            tolerance,
            limits,
            || {
                polls += 1;
                if polls == split_poll {
                    Err(DyadicIntervalClosureStopV1::Cancelled)
                } else {
                    Ok(())
                }
            },
        );
    assert!(matches!(
        stopped,
        Err(DyadicIntervalClosureControlErrorV1::Cancelled)
    ));
}

#[test]
fn legacy_v1_stationary_partition_and_binding_remain_unchanged() {
    let fixture = fixture();
    let legacy = fixture
        .geometry
        .prove_dyadic_schedule_closure_v1(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.ordinary,
            1.0e-8,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 0,
                max_leaves: 1,
                max_work: 1_000_000,
                schedule_limits: fixture.schedule_limits,
            },
        )
        .unwrap();
    let binding_before = legacy.partition_binding_fingerprint_v2();
    assert_eq!(legacy.leaves().len(), 1);
    assert!(legacy.has_canonical_complete_partition_v1());
    assert!(legacy.every_leaf_covers_graph_v1(&fixture.geometry));

    let v2 = issue(
        &fixture,
        &fixture.ordinary,
        generous_limits(fixture.schedule_limits),
    )
    .unwrap();
    assert_eq!(v2.partition(), &[(0, 0)]);
    assert_eq!(
        legacy.leaves()[0].2.checked_hinges(),
        v2.canonical_checked_hinges()
    );
    assert_eq!(binding_before, legacy.partition_binding_fingerprint_v2());
}
