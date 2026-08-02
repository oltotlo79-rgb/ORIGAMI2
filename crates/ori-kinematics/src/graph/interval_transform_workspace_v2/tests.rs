use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use super::*;

struct Fixture {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    fixed_face: FaceId,
    angle_boxes: Vec<(EdgeId, OutwardIntervalV1)>,
}

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

fn build_fixture() -> Fixture {
    let namespace = ProjectId::schema_namespace([
        0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad, 0xae, 0xaf,
        0xb0,
    ]);
    let faces = [b"registry-a", b"registry-b", b"registry-c"]
        .map(|name| FaceId::derive_v5(namespace, name));
    let edges = [b"registry-ab", b"registry-bc", b"registry-ca"]
        .map(|name| EdgeId::derive_v5(namespace, name));
    let topology = TopologySnapshot {
        source_revision: 1,
        faces: faces.iter().copied().map(face).collect(),
        edge_incidence: Vec::new(),
        hinge_adjacency: [
            (edges[0], faces[0], faces[1]),
            (edges[1], faces[1], faces[2]),
            (edges[2], faces[2], faces[0]),
        ]
        .into_iter()
        .map(|(edge, first, second)| FaceAdjacency {
            edge,
            first,
            second,
            assignment: FoldAssignment::Mountain,
        })
        .collect(),
        material_components: Vec::new(),
    };
    let audit = MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
        .expect("cycle audit");
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
    let zero = OutwardIntervalV1::new(0.0, 0.0).unwrap();
    let mut angle_boxes = edges
        .into_iter()
        .map(|edge| (edge, zero))
        .collect::<Vec<_>>();
    angle_boxes.sort_unstable_by_key(|(edge, _)| edge.canonical_bytes());
    Fixture {
        geometry,
        audit,
        fixed_face,
        angle_boxes,
    }
}

fn generous_limits() -> IntervalFaceTransformWorkspaceLimitsV2 {
    IntervalFaceTransformWorkspaceLimitsV2 {
        max_work: 100_000,
        max_validation_work: 1_000_000,
        max_sort_comparisons: 1_000_000,
        max_workspace_bytes: 1_000_000,
        max_retained_bytes: 1_000_000,
    }
}

fn exact_limits(fixture: &Fixture) -> IntervalFaceTransformWorkspaceLimitsV2 {
    let bound = fixture
        .geometry
        .checked_interval_face_transform_workspace_bound_with_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            generous_limits(),
            || Ok(()),
        )
        .unwrap();
    let resources = bound.checked_resources();
    IntervalFaceTransformWorkspaceLimitsV2 {
        max_work: generous_limits().max_work,
        max_validation_work: resources.validation_work_upper_bound(),
        max_sort_comparisons: resources.sort_comparison_upper_bound(),
        max_workspace_bytes: resources.construction_peak_bytes(),
        max_retained_bytes: resources.retained_registry_bytes(),
    }
}

#[test]
fn exact_caps_physical_capacity_foreign_binding_and_inconclusive_closure() {
    let fixture = build_fixture();
    let limits = exact_limits(&fixture);
    let bound = fixture
        .geometry
        .checked_interval_face_transform_workspace_bound_with_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            limits,
            || Ok(()),
        )
        .expect("exact bound");
    let checked = bound.checked_resources();
    let registry = fixture
        .geometry
        .prepare_interval_face_transform_registry_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.angle_boxes,
            0.0,
            &bound,
            || Ok(()),
        )
        .expect("exact physical capacities");
    let bound_debug = format!("{bound:?}");
    assert!(!bound_debug.contains("audit_binding"));
    assert!(!bound_debug.contains("issuer_geometry"));
    let registry_debug = format!("{registry:?}");
    assert!(!registry_debug.contains("poses"));
    assert!(!registry_debug.contains("input_binding"));
    assert!(!registry_debug.contains("issuer_geometry"));
    assert_eq!(registry.resources(), checked);
    assert!(registry.matches_binding_v2(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &fixture.angle_boxes,
        0.0,
        limits.max_work,
    ));
    for (position, face) in fixture.geometry.face_ids().iter().copied().enumerate() {
        assert!(
            registry
                .transform_for_canonical_face_position_v2(&fixture.geometry, position, face)
                .is_some()
        );
    }

    let foreign = build_fixture();
    assert!(!registry.matches_binding_v2(
        &foreign.geometry,
        &foreign.audit,
        foreign.fixed_face,
        &foreign.angle_boxes,
        0.0,
        limits.max_work,
    ));
    assert_eq!(
        foreign
            .geometry
            .prepare_interval_face_transform_registry_with_workspace_and_checkpoint_v2(
                &foreign.audit,
                foreign.fixed_face,
                &foreign.angle_boxes,
                0.0,
                &bound,
                || Ok(()),
            )
            .unwrap_err(),
        IntervalFaceTransformWorkspaceErrorV2::InvalidInput
    );

    let mut inconsistent = fixture.angle_boxes.clone();
    let closure_edge = fixture.audit.closure_hinges()[0];
    inconsistent
        .iter_mut()
        .find(|(edge, _)| *edge == closure_edge)
        .unwrap()
        .1 = OutwardIntervalV1::new(10.0, 10.0).unwrap();
    assert_eq!(
        fixture
            .geometry
            .prepare_interval_face_transform_registry_with_workspace_and_checkpoint_v2(
                &fixture.audit,
                fixture.fixed_face,
                &inconsistent,
                0.0,
                &bound,
                || Ok(()),
            )
            .unwrap_err(),
        IntervalFaceTransformWorkspaceErrorV2::Unproven
    );
    assert!(!registry.matches_binding_v2(
        &fixture.geometry,
        &fixture.audit,
        fixture.fixed_face,
        &inconsistent,
        0.0,
        limits.max_work,
    ));

    for invalid_angles in [
        fixture.angle_boxes[..fixture.angle_boxes.len() - 1].to_vec(),
        {
            let mut values = fixture.angle_boxes.clone();
            values.swap(0, 1);
            values
        },
        {
            let mut values = fixture.angle_boxes.clone();
            values[0].0 = EdgeId::derive_v5(
                ProjectId::schema_namespace([0xcc; 16]),
                b"foreign-registry-edge",
            );
            values
        },
    ] {
        assert_eq!(
            fixture
                .geometry
                .prepare_interval_face_transform_registry_with_workspace_and_checkpoint_v2(
                    &fixture.audit,
                    fixture.fixed_face,
                    &invalid_angles,
                    0.0,
                    &bound,
                    || Ok(()),
                )
                .unwrap_err(),
            IntervalFaceTransformWorkspaceErrorV2::InvalidInput
        );
    }

    let mut different_audit = fixture.audit.clone();
    std::mem::swap(
        &mut different_audit.spanning_hinges[0],
        &mut different_audit.closure_hinges[0],
    );
    different_audit
        .spanning_hinges
        .sort_unstable_by_key(EdgeId::canonical_bytes);
    different_audit
        .closure_hinges
        .sort_unstable_by_key(EdgeId::canonical_bytes);
    assert_eq!(
        fixture
            .geometry
            .prepare_interval_face_transform_registry_with_workspace_and_checkpoint_v2(
                &different_audit,
                fixture.fixed_face,
                &fixture.angle_boxes,
                0.0,
                &bound,
                || Ok(()),
            )
            .unwrap_err(),
        IntervalFaceTransformWorkspaceErrorV2::InvalidInput
    );
}

#[test]
fn every_checked_cap_is_exact_one_short_overflow_and_all_closure_fail_closed() {
    let fixture = build_fixture();
    let exact = exact_limits(&fixture);
    let mut early_polls = 0usize;
    assert_eq!(
        fixture
            .geometry
            .checked_interval_face_transform_workspace_bound_with_checkpoint_v2(
                &fixture.audit,
                fixture.fixed_face,
                IntervalFaceTransformWorkspaceLimitsV2 {
                    max_validation_work: exact.max_validation_work - 1,
                    ..exact
                },
                || {
                    early_polls += 1;
                    Ok(())
                },
            )
            .unwrap_err(),
        IntervalFaceTransformWorkspaceErrorV2::ResourceLimit
    );
    assert_eq!(early_polls, 1, "hard caps precede carrier scans");
    for one_short in [
        IntervalFaceTransformWorkspaceLimitsV2 {
            max_validation_work: exact.max_validation_work - 1,
            ..exact
        },
        IntervalFaceTransformWorkspaceLimitsV2 {
            max_sort_comparisons: exact.max_sort_comparisons - 1,
            ..exact
        },
        IntervalFaceTransformWorkspaceLimitsV2 {
            max_workspace_bytes: exact.max_workspace_bytes - 1,
            ..exact
        },
        IntervalFaceTransformWorkspaceLimitsV2 {
            max_retained_bytes: exact.max_retained_bytes - 1,
            ..exact
        },
    ] {
        assert_eq!(
            fixture
                .geometry
                .checked_interval_face_transform_workspace_bound_with_checkpoint_v2(
                    &fixture.audit,
                    fixture.fixed_face,
                    one_short,
                    || Ok(()),
                )
                .unwrap_err(),
            IntervalFaceTransformWorkspaceErrorV2::ResourceLimit
        );
    }
    for overflow in [
        IntervalFaceTransformWorkspaceLimitsV2 {
            max_work: usize::MAX,
            ..exact
        },
        IntervalFaceTransformWorkspaceLimitsV2 {
            max_validation_work: usize::MAX,
            ..exact
        },
        IntervalFaceTransformWorkspaceLimitsV2 {
            max_sort_comparisons: usize::MAX,
            ..exact
        },
        IntervalFaceTransformWorkspaceLimitsV2 {
            max_workspace_bytes: usize::MAX,
            ..exact
        },
        IntervalFaceTransformWorkspaceLimitsV2 {
            max_retained_bytes: usize::MAX,
            ..exact
        },
    ] {
        assert_eq!(
            fixture
                .geometry
                .checked_interval_face_transform_workspace_bound_with_checkpoint_v2(
                    &fixture.audit,
                    fixture.fixed_face,
                    overflow,
                    || Ok(()),
                )
                .unwrap_err(),
            IntervalFaceTransformWorkspaceErrorV2::ResourceLimit
        );
    }

    let mut workspace_short_bound = fixture
        .geometry
        .checked_interval_face_transform_workspace_bound_with_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            generous_limits(),
            || Ok(()),
        )
        .unwrap();
    workspace_short_bound.limits.max_workspace_bytes = exact.max_workspace_bytes - 1;
    assert_eq!(
        fixture
            .geometry
            .prepare_interval_face_transform_registry_with_workspace_and_checkpoint_v2(
                &fixture.audit,
                fixture.fixed_face,
                &fixture.angle_boxes,
                0.0,
                &workspace_short_bound,
                || Ok(()),
            )
            .unwrap_err(),
        IntervalFaceTransformWorkspaceErrorV2::ResourceLimit
    );
    let mut retained_short_bound = fixture
        .geometry
        .checked_interval_face_transform_workspace_bound_with_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            generous_limits(),
            || Ok(()),
        )
        .unwrap();
    retained_short_bound.limits.max_retained_bytes = exact.max_retained_bytes - 1;
    assert_eq!(
        fixture
            .geometry
            .prepare_interval_face_transform_registry_with_workspace_and_checkpoint_v2(
                &fixture.audit,
                fixture.fixed_face,
                &fixture.angle_boxes,
                0.0,
                &retained_short_bound,
                || Ok(()),
            )
            .unwrap_err(),
        IntervalFaceTransformWorkspaceErrorV2::ResourceLimit
    );

    let mut all_closure = fixture.audit.clone();
    let former_spanning = all_closure.spanning_hinges.clone();
    all_closure
        .closure_hinges
        .extend_from_slice(&former_spanning);
    all_closure.spanning_hinges.clear();
    all_closure
        .closure_hinges
        .sort_unstable_by_key(EdgeId::canonical_bytes);
    assert_eq!(
        fixture
            .geometry
            .checked_interval_face_transform_workspace_bound_with_checkpoint_v2(
                &all_closure,
                fixture.fixed_face,
                exact,
                || Ok(()),
            )
            .unwrap_err(),
        IntervalFaceTransformWorkspaceErrorV2::InvalidInput
    );
}

#[test]
fn entry_and_final_publication_checkpoints_take_precedence() {
    let fixture = build_fixture();
    assert_eq!(
        fixture
            .geometry
            .checked_interval_face_transform_workspace_bound_with_checkpoint_v2(
                &fixture.audit,
                fixture.fixed_face,
                IntervalFaceTransformWorkspaceLimitsV2 {
                    max_work: 0,
                    max_validation_work: 0,
                    max_sort_comparisons: 0,
                    max_workspace_bytes: 0,
                    max_retained_bytes: 0,
                },
                || Err(DyadicIntervalClosureStopV1::Cancelled),
            )
            .unwrap_err(),
        IntervalFaceTransformWorkspaceErrorV2::Cancelled
    );

    let exact = exact_limits(&fixture);
    let bound = fixture
        .geometry
        .checked_interval_face_transform_workspace_bound_with_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            exact,
            || Ok(()),
        )
        .unwrap();
    let mut successful_polls = 0usize;
    fixture
        .geometry
        .prepare_interval_face_transform_registry_with_workspace_and_checkpoint_v2(
            &fixture.audit,
            fixture.fixed_face,
            &fixture.angle_boxes,
            0.0,
            &bound,
            || {
                successful_polls += 1;
                Ok(())
            },
        )
        .unwrap();
    let mut polls = 0usize;
    assert_eq!(
        fixture
            .geometry
            .prepare_interval_face_transform_registry_with_workspace_and_checkpoint_v2(
                &fixture.audit,
                fixture.fixed_face,
                &fixture.angle_boxes,
                0.0,
                &bound,
                || {
                    polls += 1;
                    if polls == successful_polls {
                        Err(DyadicIntervalClosureStopV1::DeadlineExceeded)
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err(),
        IntervalFaceTransformWorkspaceErrorV2::DeadlineExceeded
    );
}
