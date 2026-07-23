use ori_collision::{
    EffectiveCutCollisionGeometryInputV1, EffectiveCutCollisionGeometryLimitsV1,
    EffectiveCutSourceFlatPairObservationLimitsV1, EffectiveCutStaticThicknessLimitsV1,
    diagnose_effective_cut_source_flat_pairs_v1, prepare_effective_cut_collision_geometry_v1,
    prepare_effective_cut_static_pair_registry_bridge_v1,
    prepare_effective_cut_static_thickness_prerequisite_v1,
};
use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex};
use ori_kinematics::{
    EffectiveCutRetainedFacePairRegistryLimitsV1, TreeKinematicsLimits,
    prepare_effective_cut_kinematics_diagnostic_v1,
    prepare_effective_cut_retained_face_pair_registry_v1,
};
use ori_topology::{
    FaceExtractionInput, diagnose_cut_material_component_selection_v1,
    diagnose_effective_cut_material_snapshot_v1,
};
use serde::de::DeserializeOwned;

fn id<T: DeserializeOwned>(suffix: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\"")).unwrap()
}
fn vertex(suffix: u64, x: f64, y: f64) -> Vertex {
    Vertex {
        id: id(suffix),
        position: Point2::new(x, y),
    }
}
fn edge(suffix: u64, a: &Vertex, b: &Vertex, kind: EdgeKind) -> Edge {
    Edge {
        id: id(suffix),
        start: a.id,
        end: b.id,
        kind,
    }
}
fn fixture() -> (ProjectId, Paper, CreasePattern) {
    let a = vertex(1, 0.0, 0.0);
    let b = vertex(2, 12.0, 0.0);
    let c = vertex(3, 12.0, 8.0);
    let d = vertex(4, 0.0, 8.0);
    let p = vertex(5, 2.0, 2.0);
    let q = vertex(6, 5.0, 2.0);
    let r = vertex(7, 3.5, 5.0);
    let vertices = vec![
        a.clone(),
        b.clone(),
        c.clone(),
        d.clone(),
        p.clone(),
        q.clone(),
        r.clone(),
    ];
    let edges = vec![
        edge(20, &a, &b, EdgeKind::Boundary),
        edge(21, &b, &c, EdgeKind::Boundary),
        edge(22, &c, &d, EdgeKind::Boundary),
        edge(23, &d, &a, EdgeKind::Boundary),
        edge(30, &p, &q, EdgeKind::Cut),
        edge(31, &q, &r, EdgeKind::Cut),
        edge(32, &r, &p, EdgeKind::Cut),
        edge(40, &p, &a, EdgeKind::Mountain),
        edge(41, &q, &b, EdgeKind::Valley),
    ];
    let paper = Paper {
        boundary_vertices: vec![a.id, b.id, c.id, d.id],
        cutting_allowed: true,
        thickness_mm: 0.1,
        ..Paper::default()
    };
    (id(100), paper, CreasePattern { vertices, edges })
}
fn input<'a>(
    namespace: ProjectId,
    revision: u64,
    paper: &'a Paper,
    pattern: &'a CreasePattern,
) -> FaceExtractionInput<'a> {
    FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: revision,
        paper,
        pattern,
    }
}

#[test]
fn source_flat_thickness_prerequisite_reports_planned_pair_cardinality_only() {
    let (namespace, paper, pattern) = fixture();
    let source = input(namespace, 9, &paper, &pattern);
    let selection =
        diagnose_cut_material_component_selection_v1(source, Default::default()).unwrap();
    let removed = selection
        .selections()
        .iter()
        .filter(|entry| !entry.owns_original_boundary)
        .map(|entry| entry.component)
        .collect::<Vec<_>>();
    let effective =
        diagnose_effective_cut_material_snapshot_v1(source, &removed, Default::default()).unwrap();
    let kinematics =
        prepare_effective_cut_kinematics_diagnostic_v1(&effective, source, Default::default())
            .unwrap();
    let diagnostic = prepare_effective_cut_static_thickness_prerequisite_v1(
        &kinematics,
        &effective,
        source,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let registry_limits = EffectiveCutRetainedFacePairRegistryLimitsV1 {
        max_pairs: 1_000_000,
        max_shared_hinge_memberships: 2,
    };
    let registry = prepare_effective_cut_retained_face_pair_registry_v1(
        &kinematics,
        &effective,
        source,
        Default::default(),
        registry_limits,
    )
    .unwrap();
    let bridge = prepare_effective_cut_static_pair_registry_bridge_v1(
        &diagnostic,
        &registry,
        &kinematics,
        &effective,
        source,
        Default::default(),
        Default::default(),
        registry_limits,
    )
    .unwrap();
    assert_eq!(bridge.pair_count(), 1);
    assert!(!bridge.authorizes_pair_classification());
    assert!(!bridge.authorizes_collision_free_classification());
    assert!(!bridge.authorizes_simulation_admission());
    assert!(!bridge.authorizes_project_mutation());
    assert!(!bridge.authorizes_material_removal());
    assert!(!bridge.authorizes_persistence());
    assert!(bridge.is_for(
        &diagnostic,
        &registry,
        &kinematics,
        &effective,
        source,
        Default::default(),
        Default::default(),
        registry_limits,
    ));
    let geometry_input = EffectiveCutCollisionGeometryInputV1 {
        bridge: &bridge,
        prerequisite: &diagnostic,
        registry: &registry,
        kinematics: &kinematics,
        effective: &effective,
        source,
        kinematics_limits: Default::default(),
        prerequisite_limits: Default::default(),
        registry_limits,
        geometry_limits: Default::default(),
    };
    let geometry = prepare_effective_cut_collision_geometry_v1(geometry_input).unwrap();
    assert_eq!(geometry.face_count(), 2);
    assert_eq!(geometry.hinge_membership_count(), 2);
    assert!(geometry.boundary_occurrence_count() > 0);
    assert!(geometry.converted_cut_boundary_occurrence_count() > 0);
    assert!(geometry.observes_source_flat_identity_only());
    assert!(!geometry.authorizes_pair_classification());
    assert!(!geometry.authorizes_collision_free_classification());
    assert!(!geometry.authorizes_pose_solving());
    assert!(!geometry.authorizes_simulation_admission());
    assert!(!geometry.authorizes_project_mutation());
    assert!(!geometry.authorizes_material_removal());
    assert!(!geometry.authorizes_persistence());
    assert!(geometry.is_for(geometry_input));
    assert!(!format!("{geometry:?}").contains("00000000-0000"));
    for limits in [
        EffectiveCutCollisionGeometryLimitsV1 {
            max_faces: 1,
            ..Default::default()
        },
        EffectiveCutCollisionGeometryLimitsV1 {
            max_boundary_vertices: geometry.boundary_occurrence_count() - 1,
            ..Default::default()
        },
        EffectiveCutCollisionGeometryLimitsV1 {
            max_hinge_memberships: 1,
            ..Default::default()
        },
    ] {
        assert!(
            prepare_effective_cut_collision_geometry_v1(EffectiveCutCollisionGeometryInputV1 {
                geometry_limits: limits,
                ..geometry_input
            })
            .is_err()
        );
    }
    let exact_geometry_limits = EffectiveCutCollisionGeometryLimitsV1 {
        max_faces: geometry.face_count(),
        max_boundary_vertices: geometry.boundary_occurrence_count(),
        max_hinge_memberships: geometry.hinge_membership_count(),
    };
    let exact = prepare_effective_cut_collision_geometry_v1(EffectiveCutCollisionGeometryInputV1 {
        geometry_limits: exact_geometry_limits,
        ..geometry_input
    })
    .unwrap();
    assert_ne!(geometry.fingerprint_v1(), exact.fingerprint_v1());
    assert!(!geometry.is_for(EffectiveCutCollisionGeometryInputV1 {
        geometry_limits: exact_geometry_limits,
        ..geometry_input
    }));
    let observation =
        diagnose_effective_cut_source_flat_pairs_v1(&geometry, geometry_input, Default::default())
            .unwrap();
    assert_eq!(observation.pair_count(), 1);
    assert_eq!(observation.indeterminate_pairs(), 1);
    assert_eq!(observation.penetrating_pairs(), 0);
    assert_eq!(observation.shared_hinge_allowed_pairs(), 0);
    assert_eq!(observation.shared_vertex_allowed_pairs(), 0);
    assert_eq!(
        observation.separated_pairs()
            + observation.touching_pairs()
            + observation.shared_hinge_allowed_pairs()
            + observation.shared_vertex_allowed_pairs()
            + observation.penetrating_pairs()
            + observation.indeterminate_pairs(),
        observation.pair_count()
    );
    assert!(!observation.authorizes_pair_classification());
    assert!(!observation.authorizes_collision_free_classification());
    assert!(!observation.authorizes_pose_solving());
    assert!(!observation.authorizes_simulation_admission());
    assert!(!observation.authorizes_project_mutation());
    assert!(!observation.authorizes_material_removal());
    assert!(!observation.authorizes_persistence());
    assert!(observation.is_for(&geometry, geometry_input, Default::default()));
    assert!(
        diagnose_effective_cut_source_flat_pairs_v1(
            &geometry,
            geometry_input,
            EffectiveCutSourceFlatPairObservationLimitsV1 {
                max_pairs: 0,
                max_shared_vertex_work: 10_000_000,
            },
        )
        .is_err()
    );
    let exact_work = geometry
        .boundary_occurrence_count()
        .checked_mul(geometry.face_count() - 1)
        .unwrap();
    let exact_observation_limits = EffectiveCutSourceFlatPairObservationLimitsV1 {
        max_pairs: 1,
        max_shared_vertex_work: exact_work,
    };
    let exact_observation = diagnose_effective_cut_source_flat_pairs_v1(
        &geometry,
        geometry_input,
        exact_observation_limits,
    )
    .unwrap();
    assert_ne!(
        observation.fingerprint_v1(),
        exact_observation.fingerprint_v1()
    );
    assert!(!observation.is_for(&geometry, geometry_input, exact_observation_limits));
    assert!(
        diagnose_effective_cut_source_flat_pairs_v1(
            &geometry,
            geometry_input,
            EffectiveCutSourceFlatPairObservationLimitsV1 {
                max_pairs: 1,
                max_shared_vertex_work: exact_work - 1,
            },
        )
        .is_err()
    );
    assert!(
        prepare_effective_cut_static_pair_registry_bridge_v1(
            &diagnostic,
            &registry,
            &kinematics,
            &effective,
            source,
            Default::default(),
            EffectiveCutStaticThicknessLimitsV1 { max_face_pairs: 1 },
            registry_limits,
        )
        .is_err()
    );
    assert!(!bridge.is_for(
        &diagnostic,
        &registry,
        &kinematics,
        &effective,
        input(namespace, 10, &paper, &pattern),
        Default::default(),
        Default::default(),
        registry_limits,
    ));
    assert!(!bridge.is_for(
        &diagnostic,
        &registry,
        &kinematics,
        &effective,
        source,
        Default::default(),
        Default::default(),
        EffectiveCutRetainedFacePairRegistryLimitsV1 {
            max_pairs: 1_000_000,
            max_shared_hinge_memberships: 3,
        },
    ));
    assert_eq!(diagnostic.face_count(), 2);
    assert_eq!(diagnostic.hinge_count(), 2);
    assert_eq!(diagnostic.planned_unordered_face_pair_count(), 1);
    assert!(diagnostic.observes_source_flat_convention_only());
    assert_eq!(diagnostic.paper_thickness_mm(), 0.1);
    assert!(!diagnostic.authorizes_collision_free_classification());
    assert!(!diagnostic.authorizes_simulation_admission());
    assert!(!diagnostic.authorizes_project_mutation());
    assert!(!diagnostic.authorizes_material_removal());
    assert!(!diagnostic.authorizes_persistence());
    assert!(diagnostic.is_for(
        &kinematics,
        &effective,
        source,
        Default::default(),
        Default::default(),
    ));
    assert!(
        prepare_effective_cut_static_thickness_prerequisite_v1(
            &kinematics,
            &effective,
            source,
            Default::default(),
            EffectiveCutStaticThicknessLimitsV1 { max_face_pairs: 1 },
        )
        .is_ok()
    );
    assert!(!diagnostic.is_for(
        &kinematics,
        &effective,
        source,
        TreeKinematicsLimits {
            max_faces: 2,
            ..Default::default()
        },
        Default::default(),
    ));
    assert!(!diagnostic.is_for(
        &kinematics,
        &effective,
        source,
        Default::default(),
        EffectiveCutStaticThicknessLimitsV1 { max_face_pairs: 2 },
    ));
    assert!(
        prepare_effective_cut_static_thickness_prerequisite_v1(
            &kinematics,
            &effective,
            source,
            Default::default(),
            EffectiveCutStaticThicknessLimitsV1 { max_face_pairs: 0 },
        )
        .is_err()
    );

    for invalid in [0.0, -0.0, -0.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut changed = paper.clone();
        changed.thickness_mm = invalid;
        assert!(
            prepare_effective_cut_static_thickness_prerequisite_v1(
                &kinematics,
                &effective,
                input(namespace, 9, &changed, &pattern),
                TreeKinematicsLimits::default(),
                Default::default(),
            )
            .is_err()
        );
    }
    assert!(!diagnostic.is_for(
        &kinematics,
        &effective,
        input(namespace, 10, &paper, &pattern),
        Default::default(),
        Default::default(),
    ));
    let wider = prepare_effective_cut_static_thickness_prerequisite_v1(
        &kinematics,
        &effective,
        source,
        Default::default(),
        EffectiveCutStaticThicknessLimitsV1 { max_face_pairs: 2 },
    )
    .unwrap();
    assert_ne!(diagnostic.fingerprint_v1(), wider.fingerprint_v1());
    assert!(
        prepare_effective_cut_static_pair_registry_bridge_v1(
            &wider,
            &registry,
            &kinematics,
            &effective,
            source,
            Default::default(),
            Default::default(),
            registry_limits,
        )
        .is_err()
    );
}
