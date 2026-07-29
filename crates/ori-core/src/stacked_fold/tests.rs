use ori_domain::{Edge, EdgeId, EdgeKind, LengthDisplayUnit, Vertex};
use ori_foldability::{
    GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, analyze_global_flat_foldability,
};
use ori_topology::{analyze_faces, analyze_local_flat_foldability};

use super::*;
use crate::{EditorState, create_rectangular_sheet};

struct Fixture {
    identity: ProjectId,
    source_pattern: CreasePattern,
    source_paper: Paper,
    source_layer_order: LayerOrderSnapshot,
    target_pattern: CreasePattern,
    target_paper: Paper,
}

impl Fixture {
    fn input(&self) -> FaceLineageInput<'_> {
        FaceLineageInput {
            identity_namespace: self.identity,
            source_revision: 7,
            source_paper: &self.source_paper,
            source_pattern: &self.source_pattern,
            source_layer_order: &self.source_layer_order,
            target_revision: 8,
            target_paper: &self.target_paper,
            target_pattern: &self.target_pattern,
        }
    }
}

fn fixture() -> Fixture {
    let identity = ProjectId::new();
    let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
    let (source_pattern, source_paper) = sheet.into_parts();
    let source_layer_order = proven_layer_order(identity, 7, &source_pattern, &source_paper);

    let mut target_pattern = source_pattern.clone();
    target_pattern.edges.push(Edge {
        id: EdgeId::new(),
        start: source_paper.boundary_vertices[0],
        end: source_paper.boundary_vertices[2],
        kind: EdgeKind::Mountain,
    });

    Fixture {
        identity,
        source_pattern,
        source_paper: source_paper.clone(),
        source_layer_order,
        target_pattern,
        target_paper: source_paper,
    }
}

#[derive(Clone)]
struct GeometryFixture {
    identity: ProjectId,
    source_revision: Revision,
    target_revision: Revision,
    source_pattern: CreasePattern,
    source_paper: Paper,
    source_layer_order: LayerOrderSnapshot,
    target_pattern: CreasePattern,
    target_paper: Paper,
    expected_creases: Vec<ExpectedStackedFoldCreaseV1>,
}

impl GeometryFixture {
    fn lineage_input(&self) -> FaceLineageInput<'_> {
        FaceLineageInput {
            identity_namespace: self.identity,
            source_revision: self.source_revision,
            source_paper: &self.source_paper,
            source_pattern: &self.source_pattern,
            source_layer_order: &self.source_layer_order,
            target_revision: self.target_revision,
            target_paper: &self.target_paper,
            target_pattern: &self.target_pattern,
        }
    }

    fn lineage(&self) -> FaceLineageV1 {
        prepare_face_lineage_v1(self.lineage_input(), FaceLineageLimits::default())
            .expect("prepare geometry fixture lineage")
    }

    fn geometry_input<'a>(&'a self, lineage: &'a FaceLineageV1) -> StackedFoldGeometryInputV1<'a> {
        StackedFoldGeometryInputV1 {
            identity_namespace: self.identity,
            source_revision: self.source_revision,
            source_paper: &self.source_paper,
            source_pattern: &self.source_pattern,
            target_revision: self.target_revision,
            target_paper: &self.target_paper,
            target_pattern: &self.target_pattern,
            face_lineage: lineage,
            expected_creases: &self.expected_creases,
        }
    }
}

fn simple_geometry_fixture() -> GeometryFixture {
    let fixture = fixture();
    let expected_creases = vec![ExpectedStackedFoldCreaseV1 {
        start: vertex_position(
            &fixture.source_pattern,
            fixture.source_paper.boundary_vertices[0],
        ),
        end: vertex_position(
            &fixture.source_pattern,
            fixture.source_paper.boundary_vertices[2],
        ),
        kind: EdgeKind::Mountain,
    }];
    GeometryFixture {
        identity: fixture.identity,
        source_revision: 7,
        target_revision: 8,
        source_pattern: fixture.source_pattern,
        source_paper: fixture.source_paper,
        source_layer_order: fixture.source_layer_order,
        target_pattern: fixture.target_pattern,
        target_paper: fixture.target_paper,
        expected_creases,
    }
}

#[test]
fn prepared_source_pose_match_limits_cannot_relax_hard_ceilings() {
    let relaxed = PreparedStackedFoldSourcePoseMatchLimitsV1 {
        max_source_vertices: usize::MAX,
        max_source_edges: usize::MAX,
        max_source_paper_boundary_vertices: usize::MAX,
        max_source_faces: usize::MAX,
        max_source_hinges: usize::MAX,
        max_target_faces: usize::MAX,
        max_target_hinges: usize::MAX,
        max_target_edge_mapping_records: usize::MAX,
        max_total_records: usize::MAX,
    };
    assert_eq!(
        relaxed.effective(),
        PreparedStackedFoldSourcePoseMatchLimitsV1::default()
    );

    let tightened = PreparedStackedFoldSourcePoseMatchLimitsV1 {
        max_source_vertices: 1,
        max_source_edges: 2,
        max_source_paper_boundary_vertices: 3,
        max_source_faces: 4,
        max_source_hinges: 5,
        max_target_faces: 6,
        max_target_hinges: 7,
        max_target_edge_mapping_records: 8,
        max_total_records: 9,
    };
    assert_eq!(tightened.effective(), tightened);
}

#[test]
fn prepared_source_pose_match_count_helpers_admit_equality_and_fail_closed() {
    let maximum =
        PreparedStackedFoldSourcePoseMatchLimitsV1::default().max_target_edge_mapping_records;
    assert_eq!(
        check_prepared_source_pose_limit_v1(
            PreparedStackedFoldSourcePoseResourceV1::TargetEdgeMappingRecords,
            maximum,
            maximum,
        ),
        Ok(())
    );
    assert_eq!(
        check_prepared_source_pose_limit_v1(
            PreparedStackedFoldSourcePoseResourceV1::TargetEdgeMappingRecords,
            maximum + 1,
            maximum,
        ),
        Err(
            PreparedStackedFoldSourcePoseMatchErrorV1::ResourceLimitExceeded {
                resource: PreparedStackedFoldSourcePoseResourceV1::TargetEdgeMappingRecords,
                actual: maximum + 1,
                maximum,
            }
        )
    );
    assert_eq!(
        checked_add_prepared_source_pose_count_v1(
            usize::MAX,
            1,
            PreparedStackedFoldSourcePoseResourceV1::TotalRecords,
        ),
        Err(
            PreparedStackedFoldSourcePoseMatchErrorV1::ResourceCountOverflow {
                resource: PreparedStackedFoldSourcePoseResourceV1::TotalRecords,
            }
        )
    );
}

#[test]
fn expected_target_angle_mapping_rejects_duplicate_extra_and_missing_edges() {
    let mut edges = [EdgeId::new(), EdgeId::new(), EdgeId::new()];
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let actual = [(edges[0], 30.0), (edges[1], 0.0)];

    let mut exact = vec![(edges[1], 0.0), (edges[0], 30.0)];
    assert!(expected_target_angle_mapping_matches_v1(
        &mut exact,
        actual.iter().map(|(edge, angle)| (*edge, *edge, *angle)),
    ));

    let mut duplicate = vec![(edges[0], 30.0), (edges[0], 30.0)];
    assert!(!expected_target_angle_mapping_matches_v1(
        &mut duplicate,
        actual.iter().map(|(edge, angle)| (*edge, *edge, *angle)),
    ));

    let mut extra = vec![(edges[0], 30.0), (edges[1], 0.0), (edges[2], 15.0)];
    assert!(!expected_target_angle_mapping_matches_v1(
        &mut extra,
        actual.iter().map(|(edge, angle)| (*edge, *edge, *angle)),
    ));

    let mut missing = vec![(edges[0], 30.0)];
    assert!(!expected_target_angle_mapping_matches_v1(
        &mut missing,
        actual.iter().map(|(edge, angle)| (*edge, *edge, *angle)),
    ));
}

#[test]
fn expected_target_angle_mapping_is_bit_exact_for_ulp_and_signed_zero() {
    let edge = EdgeId::new();
    let actual_angle = 30.0_f64;
    let mut one_ulp_away = vec![(edge, f64::from_bits(actual_angle.to_bits() + 1))];
    assert!(!expected_target_angle_mapping_matches_v1(
        &mut one_ulp_away,
        [(edge, edge, actual_angle)].into_iter(),
    ));

    let mut positive_zero = vec![(edge, 0.0)];
    assert!(!expected_target_angle_mapping_matches_v1(
        &mut positive_zero,
        [(edge, edge, -0.0)].into_iter(),
    ));
}

#[test]
fn topology_builder_creates_provable_cross_arrangement() {
    let identity = ProjectId::new();
    let source_revision = 31;
    let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
    let (source_pattern, source_paper) = sheet.into_parts();
    let corners = source_paper
        .boundary_vertices
        .iter()
        .map(|id| vertex_position(&source_pattern, *id))
        .collect::<Vec<_>>();
    let expected = [
        ExpectedStackedFoldCreaseV1 {
            start: corners[0],
            end: corners[2],
            kind: EdgeKind::Mountain,
        },
        ExpectedStackedFoldCreaseV1 {
            start: corners[1],
            end: corners[3],
            kind: EdgeKind::Valley,
        },
    ];

    let candidate = build_stacked_fold_topology_v1(
        identity,
        source_revision,
        &source_pattern,
        &source_paper,
        &expected,
        StackedFoldTopologyBuildLimitsV1::default(),
    )
    .expect("build crossing crease arrangement");
    assert_eq!(candidate.pattern.vertices.len(), 5);
    assert_eq!(candidate.pattern.edges.len(), 8);
    assert_eq!(
        candidate.paper.boundary_vertices,
        source_paper.boundary_vertices
    );
    let mut reversed_expected = expected;
    reversed_expected.reverse();
    for crease in &mut reversed_expected {
        std::mem::swap(&mut crease.start, &mut crease.end);
    }
    let repeated = build_stacked_fold_topology_v1(
        identity,
        source_revision,
        &source_pattern,
        &source_paper,
        &reversed_expected,
        StackedFoldTopologyBuildLimitsV1::default(),
    )
    .expect("repeat with reversed caller order and direction");
    assert_eq!(candidate, repeated);
    let mut signed_zero_expected = expected;
    for crease in &mut signed_zero_expected {
        for coordinate in [
            &mut crease.start.x,
            &mut crease.start.y,
            &mut crease.end.x,
            &mut crease.end.y,
        ] {
            if *coordinate == 0.0 {
                *coordinate = -0.0;
            }
        }
    }
    let signed_zero = build_stacked_fold_topology_v1(
        identity,
        source_revision,
        &source_pattern,
        &source_paper,
        &signed_zero_expected,
        StackedFoldTopologyBuildLimitsV1::default(),
    )
    .expect("canonicalize signed zero");
    assert_eq!(candidate, signed_zero);

    let source_layer_order =
        proven_layer_order(identity, source_revision, &source_pattern, &source_paper);
    let lineage = prepare_face_lineage_v1(
        FaceLineageInput {
            identity_namespace: identity,
            source_revision,
            source_paper: &source_paper,
            source_pattern: &source_pattern,
            source_layer_order: &source_layer_order,
            target_revision: source_revision + 1,
            target_paper: &candidate.paper,
            target_pattern: &candidate.pattern,
        },
        FaceLineageLimits::default(),
    )
    .expect("prove generated face lineage");
    let proof = prepare_stacked_fold_geometry_v1(
        StackedFoldGeometryInputV1 {
            identity_namespace: identity,
            source_revision,
            source_paper: &source_paper,
            source_pattern: &source_pattern,
            target_revision: source_revision + 1,
            target_paper: &candidate.paper,
            target_pattern: &candidate.pattern,
            face_lineage: &lineage,
            expected_creases: &expected,
        },
        StackedFoldGeometryLimitsV1::default(),
    )
    .expect("prove generated geometry");
    assert_eq!(proof.expected_creases().len(), 2);
    assert_eq!(
        proof
            .expected_creases()
            .iter()
            .map(|crease| crease.target_edges().len())
            .collect::<Vec<_>>(),
        vec![2, 2]
    );
    let prepared = prepare_stacked_fold_geometry_candidate_v1(
        identity,
        source_revision,
        &source_pattern,
        &source_paper,
        &source_layer_order,
        &expected,
        StackedFoldTopologyBuildLimitsV1::default(),
        FaceLineageLimits::default(),
        StackedFoldGeometryLimitsV1::default(),
    )
    .expect("build and prove one owning package");
    assert_eq!(prepared.candidate(), &candidate);
    assert_eq!(prepared.proof(), &proof);
    assert!(matches!(
        prepare_stacked_fold_target_model_v1(prepared, TreeKinematicsLimits::default()),
        Err(
            PrepareStackedFoldTargetModelErrorV1::CyclicTargetUnsupported {
                closure_hinge_count: 1
            }
        )
    ));
}

#[test]
fn topology_builder_splits_paper_boundary_at_crease_endpoint() {
    let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
    let (source_pattern, source_paper) = sheet.into_parts();
    let corner = vertex_position(&source_pattern, source_paper.boundary_vertices[0]);
    let opposite = vertex_position(&source_pattern, source_paper.boundary_vertices[2]);
    let expected = [ExpectedStackedFoldCreaseV1 {
        start: Point2::new((corner.x + opposite.x) * 0.5, corner.y),
        end: Point2::new((corner.x + opposite.x) * 0.5, opposite.y),
        kind: EdgeKind::Mountain,
    }];

    let candidate = build_stacked_fold_topology_v1(
        ProjectId::new(),
        0,
        &source_pattern,
        &source_paper,
        &expected,
        StackedFoldTopologyBuildLimitsV1::default(),
    )
    .expect("build boundary-to-boundary crease");
    assert_eq!(candidate.pattern.vertices.len(), 6);
    assert_eq!(candidate.pattern.edges.len(), 7);
    assert_eq!(candidate.paper.boundary_vertices.len(), 6);
}

#[test]
fn target_graph_audit_transports_cycle_constraints_without_authority() {
    let identity = ProjectId::new();
    let source_revision = 41;
    let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
    let (source_pattern, source_paper) = sheet.into_parts();
    let corners = source_paper
        .boundary_vertices
        .iter()
        .map(|id| vertex_position(&source_pattern, *id))
        .collect::<Vec<_>>();
    let expected = [
        ExpectedStackedFoldCreaseV1 {
            start: corners[0],
            end: corners[2],
            kind: EdgeKind::Mountain,
        },
        ExpectedStackedFoldCreaseV1 {
            start: corners[1],
            end: corners[3],
            kind: EdgeKind::Valley,
        },
    ];
    let source_layer_order =
        proven_layer_order(identity, source_revision, &source_pattern, &source_paper);
    let prepare_geometry = || {
        prepare_stacked_fold_geometry_candidate_v1(
            identity,
            source_revision,
            &source_pattern,
            &source_paper,
            &source_layer_order,
            &expected,
            StackedFoldTopologyBuildLimitsV1::default(),
            FaceLineageLimits::default(),
            StackedFoldGeometryLimitsV1::default(),
        )
        .expect("prepare cyclic geometry")
    };

    let package = prepare_stacked_fold_target_graph_audit_v1(
        prepare_geometry(),
        TreeKinematicsLimits::default(),
    )
    .expect("retain cycle audit");
    assert_eq!(
        package.model_id(),
        STACKED_FOLD_TARGET_GRAPH_AUDIT_MODEL_ID_V1
    );
    assert_eq!(package.audit().faces().len(), 4);
    assert_eq!(package.audit().spanning_hinges().len(), 3);
    assert_eq!(package.audit().closure_hinges().len(), 1);
    assert_eq!(package.hinge_geometry().face_ids().len(), 4);
    assert_eq!(package.hinge_geometry().hinges().len(), 4);
    assert!(package.requires_closure_certificate());
    assert!(!package.authorizes_pose());
    assert!(!package.authorizes_apply_stacked_fold());
    assert_eq!(package.geometry().proof().expected_creases().len(), 2);
    let source_topology = simulation_snapshot(
        identity,
        source_revision,
        &source_paper,
        &source_pattern,
        FaceLineageTopology::Source,
    )
    .expect("source topology");
    let source_model = MaterialTreeKinematicsModel::prepare(
        &source_pattern,
        &source_paper,
        &source_topology,
        TreeKinematicsLimits::default(),
    )
    .expect("source model");
    let source_pose = source_model
        .solve(
            None,
            &CanonicalHingeAngles::new(Vec::new()).expect("empty angles"),
        )
        .expect("source pose");
    let prepare_initial = || {
        let target = prepare_stacked_fold_target_graph_audit_v1(
            prepare_geometry(),
            TreeKinematicsLimits::default(),
        )
        .expect("rebuild cyclic target graph");
        prepare_stacked_fold_initial_graph_pose_v1(target, &source_model, &source_pose)
            .expect("rebuild cycle initial embedding")
    };
    let prepare_stationary_schedule =
        |target: &PreparedStackedFoldTargetGraphAuditV1, pose: &ClosedMaterialHingeGraphPose| {
            let entries = pose
                .hinge_angles()
                .as_slice()
                .iter()
                .map(|angle| ori_kinematics::CycleScheduleEntryInputV1 {
                    edge: angle.edge(),
                    initial_angle_degrees_bits: angle.angle_degrees().to_bits(),
                    chebyshev_coefficients: vec![ori_kinematics::RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    }],
                })
                .collect::<Vec<_>>();
            ori_kinematics::CanonicalCycleScheduleV1::prepare(
                target.hinge_geometry(),
                target.audit(),
                pose.fixed_face(),
                [0.0, 1.0],
                entries,
                ori_kinematics::CycleScheduleLimitsV1::default(),
            )
            .expect("prepare stationary cycle schedule")
        };
    let closure_limits = ori_kinematics::DyadicIntervalClosureLimitsV1 {
        max_depth: 0,
        max_leaves: 1,
        max_work: 1,
        schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
    };
    let initial = prepare_stacked_fold_initial_graph_pose_v1(package, &source_model, &source_pose)
        .expect("cycle initial embedding closes");
    assert_eq!(
        initial.pose().closure_certificate().checked_hinges().len(),
        4
    );
    assert_eq!(initial.pose().transforms().len(), 4);
    let schedule = prepare_stationary_schedule(initial.target(), initial.pose());
    let derivative_bounds = initial
        .target()
        .hinge_geometry()
        .hinges()
        .iter()
        .map(|hinge| schedule.derivative_bound(hinge.edge()))
        .collect::<Vec<_>>();
    assert!(
        derivative_bounds
            .iter()
            .all(|bound| bound.is_some_and(|bound| bound.to_bits() == 0.0_f64.to_bits())),
        "{derivative_bounds:?}"
    );
    let scheduled_angles = schedule
        .evaluate(0.0)
        .expect("evaluate the stationary schedule");
    assert_eq!(&scheduled_angles, initial.pose().hinge_angles());
    initial
        .target()
        .hinge_geometry()
        .solve_closed(
            initial.target().audit(),
            initial.pose().fixed_face(),
            &scheduled_angles,
            STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
        )
        .expect("the stationary schedule stays closed");
    let closure = initial
        .target()
        .hinge_geometry()
        .prove_dyadic_schedule_closure_v1(
            initial.target().audit(),
            initial.pose().fixed_face(),
            &schedule,
            STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
            closure_limits,
        )
        .expect("prove the complete stationary schedule");
    assert!(closure.has_canonical_complete_partition_v1());
    assert!(closure.every_leaf_covers_graph_v1(initial.target().hinge_geometry()));
    let requested_angles = initial.pose().hinge_angles().clone();
    let scheduled = prepare_stacked_fold_requested_scheduled_graph_pose_v1(
        initial,
        &schedule,
        &closure,
        requested_angles,
        0.0,
    )
    .expect("accept a complete closure from the live graph instance");
    assert_eq!(scheduled.requested_angle_degrees(), 0.0);

    let stale_initial = prepare_initial();
    let live_schedule = prepare_stationary_schedule(stale_initial.target(), stale_initial.pose());
    let foreign_target = prepare_stacked_fold_target_graph_audit_v1(
        prepare_geometry(),
        TreeKinematicsLimits::default(),
    )
    .expect("prepare identical foreign cyclic target graph");
    assert!(
        !foreign_target
            .hinge_geometry()
            .same_instance(stale_initial.target().hinge_geometry())
    );
    let foreign_schedule = prepare_stationary_schedule(&foreign_target, stale_initial.pose());
    assert_eq!(
        foreign_schedule.graph_binding_fingerprint_v1(),
        live_schedule.graph_binding_fingerprint_v1()
    );
    assert_eq!(
        foreign_schedule.certificate_binding_fingerprint_v2(),
        live_schedule.certificate_binding_fingerprint_v2()
    );
    let foreign_closure = foreign_target
        .hinge_geometry()
        .prove_dyadic_schedule_closure_v1(
            foreign_target.audit(),
            stale_initial.pose().fixed_face(),
            &foreign_schedule,
            STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1,
            closure_limits,
        )
        .expect("prove identical content on a foreign graph instance");
    assert!(foreign_closure.every_leaf_covers_graph_v1(foreign_target.hinge_geometry()));
    assert!(!foreign_closure.every_leaf_covers_graph_v1(stale_initial.target().hinge_geometry()));
    let stale_requested_angles = stale_initial.pose().hinge_angles().clone();
    assert!(matches!(
        prepare_stacked_fold_requested_scheduled_graph_pose_v1(
            stale_initial,
            &live_schedule,
            &foreign_closure,
            stale_requested_angles,
            0.0,
        ),
        Err(PrepareStackedFoldRequestedPoseErrorV1::InvalidRequestedAngle)
    ));

    assert!(matches!(
        prepare_stacked_fold_requested_graph_pose_v1(prepare_initial(), 90.0),
        Err(PrepareStackedFoldRequestedPoseErrorV1::Kinematics(
            KinematicsError::UnsupportedTopology
        ))
    ));

    let limited = TreeKinematicsLimits {
        max_faces: 3,
        ..TreeKinematicsLimits::default()
    };
    assert!(matches!(
        prepare_stacked_fold_target_graph_audit_v1(prepare_geometry(), limited),
        Err(PrepareStackedFoldTargetGraphAuditErrorV1::ResourceLimit)
    ));
}

#[test]
fn split_existing_fold_cycle_accepts_roundoff_bounded_flat_request() {
    let identity = ProjectId::new();
    let source_revision = 52;
    let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
    let (flat_pattern, flat_paper) = sheet.into_parts();
    let corners = flat_paper
        .boundary_vertices
        .iter()
        .map(|id| vertex_position(&flat_pattern, *id))
        .collect::<Vec<_>>();
    let existing = [ExpectedStackedFoldCreaseV1 {
        start: corners[0],
        end: corners[2],
        kind: EdgeKind::Mountain,
    }];
    let source = build_stacked_fold_topology_v1(
        identity,
        source_revision - 1,
        &flat_pattern,
        &flat_paper,
        &existing,
        StackedFoldTopologyBuildLimitsV1::default(),
    )
    .expect("build source diagonal");
    let source_pattern = source.pattern;
    let source_paper = source.paper;
    let source_layer_order =
        proven_layer_order(identity, source_revision, &source_pattern, &source_paper);
    let center = Point2::new(
        (corners[0].x + corners[2].x) / 2.0,
        (corners[0].y + corners[2].y) / 2.0,
    );
    let expected = [
        ExpectedStackedFoldCreaseV1 {
            start: corners[1],
            end: center,
            kind: EdgeKind::Mountain,
        },
        ExpectedStackedFoldCreaseV1 {
            start: center,
            end: corners[3],
            kind: EdgeKind::Valley,
        },
    ];
    let geometry = prepare_stacked_fold_geometry_candidate_v1(
        identity,
        source_revision,
        &source_pattern,
        &source_paper,
        &source_layer_order,
        &expected,
        StackedFoldTopologyBuildLimitsV1::default(),
        FaceLineageLimits::default(),
        StackedFoldGeometryLimitsV1::default(),
    )
    .expect("prepare split-cycle geometry");
    let source_subdivision = geometry
        .proof()
        .source_edges()
        .iter()
        .find(|edge| edge.target_edges().len() == 2)
        .expect("the pre-existing source hinge is split exactly once");
    assert!(
        source_pattern
            .edges
            .iter()
            .any(|edge| edge.id == source_subdivision.source_edge())
    );
    assert_eq!(geometry.proof().expected_creases().len(), 2);
    assert!(
        geometry
            .proof()
            .expected_creases()
            .iter()
            .all(|crease| crease.target_edges().len() == 1)
    );
    let target =
        prepare_stacked_fold_target_graph_audit_v1(geometry, TreeKinematicsLimits::default())
            .expect("audit split cycle");
    assert!(target.requires_closure_certificate());
    let source_topology = simulation_snapshot(
        identity,
        source_revision,
        &source_paper,
        &source_pattern,
        FaceLineageTopology::Source,
    )
    .expect("source topology");
    let source_model = MaterialTreeKinematicsModel::prepare(
        &source_pattern,
        &source_paper,
        &source_topology,
        TreeKinematicsLimits::default(),
    )
    .expect("source model");
    let source_edge = source_model.hinges()[0].edge();
    let current_non_flat = revalidate_current_non_flat_layer_order_v1(
        identity,
        source_revision,
        &source_pattern,
        &source_paper,
        Some(source_model.face_ids()[0]),
        &CanonicalHingeAngles::new(vec![HingeAngle::new(source_edge, 90.0).unwrap()]).unwrap(),
        &source_layer_order,
        1,
    )
    .expect("fresh current non-flat authority");
    assert_eq!(current_non_flat.target_revision(), source_revision);
    assert_eq!(current_non_flat.folded_faces().len(), 2);
    assert_eq!(current_non_flat.tested_face_pairs(), 1);
    let mut stale_flat = source_layer_order.clone();
    stale_flat.provenance.source.source_revision += 1;
    assert_eq!(
        revalidate_current_non_flat_layer_order_v1(
            identity,
            source_revision,
            &source_pattern,
            &source_paper,
            Some(source_model.face_ids()[0]),
            &CanonicalHingeAngles::new(vec![HingeAngle::new(source_edge, 90.0).unwrap()]).unwrap(),
            &stale_flat,
            1,
        ),
        Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::SourceLayerOrderMismatch)
    );
    assert_eq!(
        revalidate_current_non_flat_layer_order_v1(
            identity,
            source_revision,
            &source_pattern,
            &source_paper,
            Some(source_model.face_ids()[0]),
            &CanonicalHingeAngles::new(vec![HingeAngle::new(source_edge, 90.0).unwrap()]).unwrap(),
            &source_layer_order,
            0,
        ),
        Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::ResourceLimit)
    );
    let source_angles =
        CanonicalHingeAngles::new(vec![HingeAngle::new(source_edge, 180.0).unwrap()]).unwrap();
    let source_pose = source_model
        .solve(Some(source_model.face_ids()[0]), &source_angles)
        .expect("fold source");
    let initial = prepare_stacked_fold_initial_graph_pose_v1(target, &source_model, &source_pose)
        .expect("lift folded source");
    let requested = prepare_stacked_fold_requested_graph_pose_v1(initial, 180.0)
        .expect("roundoff-bounded flat cycle endpoint");
    assert_eq!(requested.requested_angle_degrees(), 180.0);
    assert_eq!(
        requested
            .pose()
            .closure_certificate()
            .checked_hinges()
            .len(),
        4
    );
    let candidate = requested.initial().target().geometry().candidate();
    let target_flat = proven_layer_order(
        identity,
        source_revision + 1,
        &candidate.pattern,
        &candidate.paper,
    );
    let admitted = revalidate_current_graph_non_flat_layer_order_v1(
        RevalidateCurrentGraphNonFlatLayerOrderRequestV1 {
            identity_namespace: identity,
            revision: source_revision + 1,
            pattern: &candidate.pattern,
            paper: &candidate.paper,
            fixed_face: requested.pose().fixed_face(),
            hinge_angles: requested.pose().hinge_angles(),
            current_flat: &target_flat,
            expected_archive: None,
            max_face_pairs: 6,
        },
    );
    assert_eq!(
        admitted,
        Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::NotNonFlatEndpoint),
        "flat cyclic archives must not be admitted as non-flat evidence"
    );
}

#[test]
fn prepared_target_is_admitted_by_native_tree_kinematics() {
    let mut fixture = simple_geometry_fixture();
    fixture.source_paper.thickness_mm = 0.0;
    fixture.target_paper.thickness_mm = 0.0;
    fixture.source_layer_order = proven_layer_order(
        fixture.identity,
        fixture.source_revision,
        &fixture.source_pattern,
        &fixture.source_paper,
    );
    assert_eq!(
        fixture.source_paper.thickness_mm.to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(
        fixture.target_paper.thickness_mm.to_bits(),
        0.0_f64.to_bits()
    );
    let geometry = prepare_stacked_fold_geometry_candidate_v1(
        fixture.identity,
        fixture.source_revision,
        &fixture.source_pattern,
        &fixture.source_paper,
        &fixture.source_layer_order,
        &fixture.expected_creases,
        StackedFoldTopologyBuildLimitsV1::default(),
        FaceLineageLimits::default(),
        StackedFoldGeometryLimitsV1::default(),
    )
    .expect("prepare geometry");
    let audited =
        prepare_stacked_fold_target_graph_audit_v1(geometry, TreeKinematicsLimits::default())
            .expect("audit tree target");
    assert!(!audited.requires_closure_certificate());
    assert_eq!(audited.audit().closure_hinges(), &[]);
    let geometry = prepare_stacked_fold_geometry_candidate_v1(
        fixture.identity,
        fixture.source_revision,
        &fixture.source_pattern,
        &fixture.source_paper,
        &fixture.source_layer_order,
        &fixture.expected_creases,
        StackedFoldTopologyBuildLimitsV1::default(),
        FaceLineageLimits::default(),
        StackedFoldGeometryLimitsV1::default(),
    )
    .expect("prepare geometry after audit");
    let target = prepare_stacked_fold_target_model_v1(geometry, TreeKinematicsLimits::default())
        .expect("prepare target material tree");
    assert_eq!(target.model().face_ids().len(), 2);
    assert_eq!(target.model().hinges().len(), 1);
    assert_eq!(target.geometry().proof().expected_creases().len(), 1);
    let source_topology = simulation_snapshot(
        fixture.identity,
        fixture.source_revision,
        &fixture.source_paper,
        &fixture.source_pattern,
        FaceLineageTopology::Source,
    )
    .expect("source topology");
    let source_model = MaterialTreeKinematicsModel::prepare(
        &fixture.source_pattern,
        &fixture.source_paper,
        &source_topology,
        TreeKinematicsLimits::default(),
    )
    .expect("source model");
    let source_pose = source_model
        .solve(
            None,
            &CanonicalHingeAngles::new(Vec::new()).expect("empty angles"),
        )
        .expect("source pose");
    let mut initial = prepare_stacked_fold_initial_pose_v1(target, &source_model, &source_pose)
        .expect("lift source pose");
    assert!(initial.target().model().owns_pose(initial.pose()));
    assert_eq!(initial.pose().hinge_angles().len(), 1);
    assert_eq!(initial.pose().hinge_angles()[0].angle_degrees(), 0.0);
    let initial_layer_order =
        prepare_stacked_fold_initial_layer_order_v1(&initial, &fixture.source_layer_order, 1)
            .expect("prepare initial pair scan");
    assert_eq!(
        initial_layer_order.model_id(),
        STACKED_FOLD_INITIAL_LAYER_ORDER_MODEL_ID_V1
    );
    assert_eq!(initial_layer_order.tested_face_pairs(), 1);
    assert!(!initial_layer_order.authorizes_continuous_motion());
    assert!(!initial_layer_order.authorizes_project_mutation());
    for unsupported_zero_neighbor in [-0.0_f64, f64::from_bits(1)] {
        initial.target.geometry.candidate.paper.thickness_mm = unsupported_zero_neighbor;
        assert_eq!(
            prepare_stacked_fold_initial_layer_order_v1(&initial, &fixture.source_layer_order, 1,),
            Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::InitialThicknessUnsupported)
        );
    }
    initial.target.geometry.candidate.paper.thickness_mm = 0.0;
    assert!(matches!(
        ori_collision::prepare_stacked_fold_initial_sample_layer_admission_v1(
            initial.target().model(),
            initial.pose(),
            0.0,
            ori_collision::StaticCollisionLimits::default(),
            &initial_layer_order,
        ),
        Err(ori_collision::StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable)
    ));
    let requested =
        prepare_stacked_fold_requested_pose_v1(initial, 37.0).expect("solve requested pose");
    let source_applied_pose = crate::prepare_applied_pose_v1(
        source_model.face_ids(),
        &[],
        Some(source_model.face_ids()[0]),
        &[],
        crate::AppliedPoseLimitsV1::default(),
    )
    .expect("prepare exact source semantic pose");
    assert_eq!(
        prepared_stacked_fold_request_matches_applied_source_pose_v1(
            &requested,
            &fixture.source_pattern,
            &fixture.source_paper,
            &source_applied_pose,
        ),
        Ok(())
    );
    let unrelated_face = FaceId::new();
    let unrelated_pose = crate::prepare_applied_pose_v1(
        &[unrelated_face],
        &[],
        None,
        &[],
        crate::AppliedPoseLimitsV1::default(),
    )
    .expect("prepare an unrelated semantic pose");
    assert_eq!(
        prepared_stacked_fold_request_matches_applied_source_pose_v1(
            &requested,
            &fixture.source_pattern,
            &fixture.source_paper,
            &unrelated_pose,
        ),
        Err(PreparedStackedFoldSourcePoseMatchErrorV1::Mismatch)
    );
    let mut issuing_editor = crate::EditorState::with_paper(
        fixture.source_pattern.clone(),
        fixture.source_paper.clone(),
    );
    for revision in 0..fixture.source_revision {
        issuing_editor
            .execute(
                revision,
                crate::Command::UpdateProjectMemo {
                    memo: format!("source revision {}", revision + 1),
                },
            )
            .expect("advance the live editor to the prepared source revision");
    }
    issuing_editor.adopt_current_applied_pose(source_applied_pose.clone());
    assert_eq!(
        issuing_editor
            .issue_speculative_unproven_fold_token_v1(
                ProjectId::new(),
                &requested,
                &initial_layer_order,
                1,
                ProjectId::new(),
                fixture.source_paper.thickness_mm,
            )
            .err(),
        Some(SpeculativeUnprovenFoldTokenIssueErrorV1::PathDiagnosticUnavailable)
    );

    let mut front_changed = fixture.source_paper.clone();
    front_changed.front.color.red ^= 1;
    let mut back_changed = fixture.source_paper.clone();
    back_changed.back.color.blue ^= 1;
    let mut display_unit_changed = fixture.source_paper.clone();
    display_unit_changed.length_display_unit = LengthDisplayUnit::Centimeter;
    for live_paper in [front_changed, back_changed, display_unit_changed] {
        let mut presentation_drifted_editor =
            crate::EditorState::with_paper(fixture.source_pattern.clone(), live_paper);
        for revision in 0..fixture.source_revision {
            presentation_drifted_editor
                .execute(
                    revision,
                    crate::Command::UpdateProjectMemo {
                        memo: format!("source revision {}", revision + 1),
                    },
                )
                .expect("advance the presentation-drifted editor");
        }
        presentation_drifted_editor.adopt_current_applied_pose(source_applied_pose.clone());
        assert_eq!(
            presentation_drifted_editor
                .issue_speculative_unproven_fold_token_v1(
                    ProjectId::new(),
                    &requested,
                    &initial_layer_order,
                    1,
                    ProjectId::new(),
                    fixture.source_paper.thickness_mm,
                )
                .err(),
            Some(SpeculativeUnprovenFoldTokenIssueErrorV1::SourcePaperPresentationMismatch)
        );
    }

    let mut cutting_policy_changed = fixture.source_paper.clone();
    cutting_policy_changed.cutting_allowed = !cutting_policy_changed.cutting_allowed;
    let mut cutting_policy_drifted_editor =
        crate::EditorState::with_paper(fixture.source_pattern.clone(), cutting_policy_changed);
    for revision in 0..fixture.source_revision {
        cutting_policy_drifted_editor
            .execute(
                revision,
                crate::Command::UpdateProjectMemo {
                    memo: format!("source revision {}", revision + 1),
                },
            )
            .expect("advance the cutting-policy-drifted editor");
    }
    cutting_policy_drifted_editor.adopt_current_applied_pose(source_applied_pose.clone());
    assert_eq!(
        cutting_policy_drifted_editor
            .issue_speculative_unproven_fold_token_v1(
                ProjectId::new(),
                &requested,
                &initial_layer_order,
                1,
                ProjectId::new(),
                fixture.source_paper.thickness_mm,
            )
            .err(),
        Some(SpeculativeUnprovenFoldTokenIssueErrorV1::SourceGeometryFingerprintMismatch)
    );

    issuing_editor.adopt_current_applied_pose(unrelated_pose);
    assert_eq!(
        issuing_editor
            .issue_speculative_unproven_fold_token_v1(
                ProjectId::new(),
                &requested,
                &initial_layer_order,
                1,
                ProjectId::new(),
                fixture.source_paper.thickness_mm,
            )
            .err(),
        Some(SpeculativeUnprovenFoldTokenIssueErrorV1::SourceAppliedPoseMismatch)
    );
    assert!(
        requested
            .initial()
            .target()
            .model()
            .owns_pose(requested.pose())
    );
    assert_eq!(requested.requested_angle_degrees(), 37.0);
    assert_eq!(requested.pose().hinge_angles()[0].angle_degrees(), 37.0);
    let certified = ori_collision::diagnose_collective_hinge_path_from_pose_v1(
        requested.initial().target().model(),
        requested.initial().pose(),
        requested.initial().pose().hinge_angles(),
        requested.pose().hinge_angles(),
        0.0,
        ori_collision::StackedFoldPathDiagnosticLimitsV1::default(),
    )
    .expect("continuous path diagnosis");
    assert!(certified.continuous_clearance_certified());
    let non_flat_order =
        prepare_stacked_fold_non_flat_layer_order_v1(&requested, &fixture.source_layer_order, 1)
            .expect("pairwise non-coincident target supports");
    assert_eq!(
        non_flat_order.model_id(),
        STACKED_FOLD_NON_FLAT_LAYER_ORDER_MODEL_ID_V1
    );
    assert_eq!(non_flat_order.material_faces().len(), 2);
    assert_eq!(non_flat_order.folded_faces().len(), 2);
    assert!(non_flat_order.folded_faces().iter().all(|folded| {
        non_flat_order.material_faces().contains(&folded.face())
            && folded.dropped_world_axis() <= 2
            && folded.source_to_plane().m00.to_f64().is_some()
            && folded.source_to_plane().m11.to_f64().is_some()
    }));
    assert_eq!(non_flat_order.identity_namespace(), fixture.identity);
    assert_eq!(
        non_flat_order.target_revision(),
        fixture.source_revision + 1
    );
    assert_eq!(non_flat_order.fixed_face(), requested.pose().fixed_face());
    assert_eq!(
        non_flat_order.hinge_angles(),
        requested.pose().hinge_angles()
    );
    assert_eq!(non_flat_order.tested_face_pairs(), 1);
    assert_eq!(non_flat_order.overlap_cell_count(), 0);
    assert_eq!(non_flat_order.face_pair_order_count(), 0);
    assert!(!non_flat_order.authorizes_apply_stacked_fold());

    let rebuild = || {
        let geometry = prepare_stacked_fold_geometry_candidate_v1(
            fixture.identity,
            fixture.source_revision,
            &fixture.source_pattern,
            &fixture.source_paper,
            &fixture.source_layer_order,
            &fixture.expected_creases,
            StackedFoldTopologyBuildLimitsV1::default(),
            FaceLineageLimits::default(),
            StackedFoldGeometryLimitsV1::default(),
        )
        .expect("rebuild geometry");
        let target =
            prepare_stacked_fold_target_model_v1(geometry, TreeKinematicsLimits::default())
                .expect("rebuild target");
        prepare_stacked_fold_initial_pose_v1(target, &source_model, &source_pose)
            .expect("rebuild initial pose")
    };
    for invalid in [0.0, -0.0, -1.0, 180.000_000_1, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            prepare_stacked_fold_requested_pose_v1(rebuild(), invalid),
            Err(PrepareStackedFoldRequestedPoseErrorV1::InvalidRequestedAngle)
        ));
    }
    let flat =
        prepare_stacked_fold_requested_pose_v1(rebuild(), 180.0).expect("solve flat endpoint");
    assert_eq!(
        prepare_stacked_fold_non_flat_layer_order_v1(
            &flat,
            &fixture.source_layer_order,
            usize::MAX,
        ),
        Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::NotNonFlatEndpoint)
    );
    let bounded =
        prepare_stacked_fold_requested_pose_v1(rebuild(), 90.0).expect("solve bounded endpoint");
    assert_eq!(
        prepare_stacked_fold_non_flat_layer_order_v1(&bounded, &fixture.source_layer_order, 0,),
        Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::ResourceLimit)
    );
    let authenticated = prepare_stacked_fold_requested_pose_v1(rebuild(), 90.0)
        .expect("solve authenticated endpoint");
    let mut stale_layer_order = fixture.source_layer_order.clone();
    stale_layer_order.provenance.source.source_revision += 1;
    assert_eq!(
        prepare_stacked_fold_non_flat_layer_order_v1(
            &authenticated,
            &stale_layer_order,
            usize::MAX,
        ),
        Err(PrepareStackedFoldNonFlatLayerOrderErrorV1::SourceLayerOrderMismatch)
    );
    let positive_thickness = prepare_stacked_fold_requested_pose_v1(rebuild(), 90.0)
        .expect("solve positive-thickness endpoint");
    let positive_order = prepare_stacked_fold_non_flat_layer_order_with_thickness_v1(
        &positive_thickness,
        &fixture.source_layer_order,
        0.1,
        usize::MAX,
    )
    .expect("two-face positive-thickness layer offset");
    assert_eq!(positive_order.material_faces().len(), 2);

    let target_model = requested.initial().target().model();
    let target_candidate = requested.initial().target().geometry().candidate();
    let expected_hinges = target_model
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let applied_angles = requested
        .pose()
        .hinge_angles()
        .iter()
        .map(|angle| (angle.edge(), angle.angle_degrees()))
        .collect::<Vec<_>>();
    let requested_hinge_angles =
        CanonicalHingeAngles::new(requested.pose().hinge_angles().to_vec())
            .expect("requested pose keeps canonical hinge angles");
    let current_applied_pose = crate::prepare_applied_pose_v1(
        target_model.face_ids(),
        &expected_hinges,
        requested.pose().fixed_face(),
        &applied_angles,
        crate::AppliedPoseLimitsV1::default(),
    )
    .expect("prepare exact active semantic pose");
    assert_eq!(
        current_applied_pose.model_id(),
        crate::APPLIED_POSE_MODEL_ID_V1,
        "the archive happy path exercises the native tree pose authority"
    );
    let archived_pairs = non_flat_order
        .face_pair_orders()
        .iter()
        .map(|pair| ArchivedNonFlatFacePairOrderInputV1 {
            lower_face: pair.lower_face(),
            upper_face: pair.upper_face(),
        })
        .collect::<Vec<_>>();
    let prepare_archive = |source_revision,
                           target_admission_revision,
                           pairs: &[ArchivedNonFlatFacePairOrderInputV1],
                           max_face_pairs| {
        prepare_archived_refined_non_flat_layer_order_v1(
            PrepareArchivedRefinedNonFlatLayerOrderRequestV1 {
                identity_namespace: fixture.identity,
                source_revision,
                source_pattern: &fixture.source_pattern,
                source_paper: &fixture.source_paper,
                target_admission_revision,
                target_pattern: &target_candidate.pattern,
                target_paper: &target_candidate.paper,
                fixed_face: requested.pose().fixed_face(),
                hinge_angles: &requested_hinge_angles,
                archived_pair_orders: pairs,
                lineage_limits: FaceLineageLimits::default(),
                geometry_limits: StackedFoldGeometryLimitsV1::default(),
                max_face_pairs,
            },
        )
    };
    let prepared = prepare_archive(fixture.source_revision, 91, &archived_pairs, usize::MAX)
        .expect("prepare opaque archive rebind package");
    assert_eq!(prepared.source_revision, fixture.source_revision);
    assert_eq!(
        prepared.ephemeral_target_revision,
        fixture.source_revision + 1
    );
    assert_eq!(prepared.target_admission_revision, 91);
    assert!(prepared.required_source_pair_orders().is_empty());
    let rebound = finish_archived_refined_non_flat_layer_order_v1(
        prepared.clone(),
        &fixture.source_layer_order,
        &current_applied_pose,
    )
    .expect("finish a freshly reconstructed archive rebind");
    assert_eq!(rebound.target_revision(), 91);
    assert_eq!(rebound.face_pair_order_count(), archived_pairs.len());
    assert_eq!(
        rebound.hinge_angles(),
        requested.pose().hinge_angles(),
        "the rebound proof retains the freshly reconstructed pose"
    );

    let mut changed_angles = applied_angles.clone();
    changed_angles[0].1 = f64::from_bits(changed_angles[0].1.to_bits() + 1);
    let changed_applied_pose = crate::prepare_applied_pose_v1(
        target_model.face_ids(),
        &expected_hinges,
        requested.pose().fixed_face(),
        &changed_angles,
        crate::AppliedPoseLimitsV1::default(),
    )
    .expect("prepare an individually valid but different semantic pose");
    assert_eq!(
        finish_archived_refined_non_flat_layer_order_v1(
            prepared,
            &fixture.source_layer_order,
            &changed_applied_pose,
        ),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::CurrentAppliedPoseMismatch)
    );

    let [first_face, second_face] = target_model.face_ids() else {
        panic!("the fixture has exactly two target faces");
    };
    let non_overlap = ArchivedNonFlatFacePairOrderInputV1 {
        lower_face: *first_face,
        upper_face: *second_face,
    };
    assert!(matches!(
        prepare_archive(fixture.source_revision, 91, &[non_overlap], usize::MAX),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::NonOverlappingTargetPair { .. })
    ));
    assert!(matches!(
        prepare_archive(
            fixture.source_revision,
            91,
            &[ArchivedNonFlatFacePairOrderInputV1 {
                lower_face: *first_face,
                upper_face: *first_face,
            }],
            usize::MAX,
        ),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::EqualTargetFace { .. })
    ));
    assert!(matches!(
        prepare_archive(
            fixture.source_revision,
            91,
            &[ArchivedNonFlatFacePairOrderInputV1 {
                lower_face: FaceId::new(),
                upper_face: *second_face,
            }],
            usize::MAX,
        ),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::UnknownTargetFace { .. })
    ));
    assert!(matches!(
        prepare_archive(
            fixture.source_revision,
            91,
            &[non_overlap, non_overlap],
            usize::MAX
        ),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::DuplicateTargetPair { .. })
    ));
    assert!(matches!(
        prepare_archive(
            fixture.source_revision,
            91,
            &[
                non_overlap,
                ArchivedNonFlatFacePairOrderInputV1 {
                    lower_face: non_overlap.upper_face,
                    upper_face: non_overlap.lower_face,
                },
            ],
            usize::MAX,
        ),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::ConflictingTargetPair { .. })
    ));
    assert!(matches!(
        prepare_archive(fixture.source_revision, 91, &[non_overlap, non_overlap], 1),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::ArchivedPairResourceLimit)
    ));
    assert!(matches!(
        prepare_archive(MAX_REVISION, 91, &archived_pairs, usize::MAX,),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::SourceRevisionCannotAdvance)
    ));
    assert!(matches!(
        prepare_archive(
            fixture.source_revision,
            MAX_REVISION + 1,
            &archived_pairs,
            usize::MAX,
        ),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::TargetAdmissionRevisionOutOfRange)
    ));
}

#[test]
fn projected_parallel_support_overlap_uses_common_bounded_frame() {
    let point = |x, y, z| Point3::new(x, y, z).expect("finite point");
    let normal = point(0.0, 1.0, 0.0);
    let first = (
        point(0.0, 0.0, 0.0),
        normal,
        vec![
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 0.0, -2.0),
            point(0.0, 0.0, -2.0),
        ],
    );
    let offset = (
        point(1.0, 3.0, -1.0),
        normal,
        vec![
            point(1.0, 3.0, -1.0),
            point(3.0, 3.0, -1.0),
            point(3.0, 3.0, -3.0),
            point(1.0, 3.0, -3.0),
        ],
    );
    let (overlap, exact_overlap, positive_area) =
        projected_convex_overlap(&first, &offset).expect("bounded overlap");
    assert_eq!(overlap.len(), 4);
    assert_eq!(exact_overlap.len(), overlap.len());
    assert!(positive_area);
    assert!((polygon_double_area(&overlap).abs() - 2.0).abs() <= 1.0e-12);
    assert_eq!(point_dot(point_delta(offset.0, first.0), first.1), 3.0);
    for (point, exact) in overlap.into_iter().zip(exact_overlap) {
        assert_eq!(exact.x.to_f64().unwrap().to_bits(), point.x.to_bits());
        assert_eq!(exact.y.to_f64().unwrap().to_bits(), point.y.to_bits());
    }
    for value in [0.0, -0.0, 0.1, -3.0, f64::MIN_POSITIVE, f64::from_bits(1)] {
        let exact = exact_rational_from_f64(value);
        let expected = if value == 0.0 { 0.0 } else { value };
        assert_eq!(exact.to_f64().unwrap().to_bits(), expected.to_bits());
    }
}

#[test]
fn archived_target_pair_classification_maps_deduplicates_and_checks_directions() {
    let face = |key| LayerFace {
        face_id: FaceId::new(),
        face_key: ori_topology::FaceKey([key; 32]),
    };
    let source_a = face(1);
    let source_b = face(2);
    let target_a1 = face(11);
    let target_a2 = face(12);
    let target_b1 = face(21);
    let target_b2 = face(22);
    let lineage = FaceLineageV1 {
        identity_namespace: ProjectId::new(),
        source_revision: 1,
        target_revision: 2,
        source_fingerprint: FoldModelFingerprintV1([1; 32]),
        target_fingerprint: FoldModelFingerprintV1([2; 32]),
        records: vec![
            FaceLineageRecord {
                source: source_a,
                descendants: vec![target_a1, target_a2],
            },
            FaceLineageRecord {
                source: source_b,
                descendants: vec![target_b1, target_b2],
            },
        ],
    };
    let overlap = |first_face, second_face, separation| ReconstructedRefinedTargetOverlapV1 {
        boundary: Vec::new(),
        exact_boundary: Vec::new(),
        first_face,
        second_face,
        separation,
    };
    let target = ReconstructedRefinedTargetV1 {
        pose_model_id: APPLIED_POSE_MODEL_ID_V1,
        fixed_face: None,
        hinge_angles: Vec::new(),
        folded_faces: Vec::new(),
        material_faces: vec![target_a1, target_a2, target_b1, target_b2],
        tested_face_pairs: 6,
        overlaps: vec![
            overlap(target_a1.face_id, target_b1.face_id, 0.0),
            overlap(target_a2.face_id, target_b2.face_id, 0.0),
            overlap(target_a1.face_id, target_b2.face_id, 2.0),
        ],
    };
    let archived = [
        ArchivedNonFlatFacePairOrderInputV1 {
            lower_face: target_a1.face_id,
            upper_face: target_b1.face_id,
        },
        ArchivedNonFlatFacePairOrderInputV1 {
            lower_face: target_a2.face_id,
            upper_face: target_b2.face_id,
        },
        ArchivedNonFlatFacePairOrderInputV1 {
            lower_face: target_a1.face_id,
            upper_face: target_b2.face_id,
        },
    ];
    let required =
        classify_archived_source_requirements_v1(&lineage, &target, &archived, archived.len())
            .expect("same-direction descendant requirements deduplicate");
    assert_eq!(
        required,
        vec![RequiredLayerOrderPair {
            lower_face: source_a,
            upper_face: source_b,
        }]
    );

    let separated_target = ReconstructedRefinedTargetV1 {
        pose_model_id: APPLIED_POSE_MODEL_ID_V1,
        fixed_face: None,
        hinge_angles: Vec::new(),
        folded_faces: Vec::new(),
        material_faces: vec![target_a1, target_a2, target_b1, target_b2],
        tested_face_pairs: 6,
        overlaps: vec![overlap(target_a1.face_id, target_b2.face_id, 2.0)],
    };
    assert!(
        classify_archived_source_requirements_v1(
            &lineage,
            &separated_target,
            &[ArchivedNonFlatFacePairOrderInputV1 {
                lower_face: target_a1.face_id,
                upper_face: target_b2.face_id,
            }],
            1,
        )
        .expect("a correctly directed separated overlap is geometry-authenticated")
        .is_empty(),
        "non-zero separation never creates a source layer-order requirement"
    );

    let mut conflicting = archived;
    let pair = &mut conflicting[1];
    std::mem::swap(&mut pair.lower_face, &mut pair.upper_face);
    assert!(matches!(
        classify_archived_source_requirements_v1(
            &lineage,
            &target,
            &conflicting,
            conflicting.len()
        ),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::ConflictingSourceRequirement { .. })
    ));

    let mut reversed_separation = archived;
    let pair = &mut reversed_separation[2];
    std::mem::swap(&mut pair.lower_face, &mut pair.upper_face);
    assert!(matches!(
        classify_archived_source_requirements_v1(
            &lineage,
            &target,
            &reversed_separation,
            reversed_separation.len()
        ),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::GeometryDirectionMismatch { .. })
    ));

    let same_source_target = ReconstructedRefinedTargetV1 {
        pose_model_id: APPLIED_POSE_MODEL_ID_V1,
        fixed_face: None,
        hinge_angles: Vec::new(),
        folded_faces: Vec::new(),
        material_faces: vec![target_a1, target_a2, target_b1, target_b2],
        tested_face_pairs: 6,
        overlaps: vec![overlap(target_a1.face_id, target_a2.face_id, 0.0)],
    };
    assert!(matches!(
        classify_archived_source_requirements_v1(
            &lineage,
            &same_source_target,
            &[ArchivedNonFlatFacePairOrderInputV1 {
                lower_face: target_a1.face_id,
                upper_face: target_a2.face_id,
            }],
            1,
        ),
        Err(
            ArchivedRefinedNonFlatLayerOrderErrorV1::CoincidentDescendantsShareSource {
                source_face
            }
        ) if source_face == source_a.face_id
    ));

    let unbound_target = face(31);
    let missing_lineage_target = ReconstructedRefinedTargetV1 {
        pose_model_id: APPLIED_POSE_MODEL_ID_V1,
        fixed_face: None,
        hinge_angles: Vec::new(),
        folded_faces: Vec::new(),
        material_faces: vec![target_a1, unbound_target],
        tested_face_pairs: 1,
        overlaps: vec![overlap(target_a1.face_id, unbound_target.face_id, 0.0)],
    };
    assert_eq!(
        classify_archived_source_requirements_v1(
            &lineage,
            &missing_lineage_target,
            &[ArchivedNonFlatFacePairOrderInputV1 {
                lower_face: target_a1.face_id,
                upper_face: unbound_target.face_id,
            }],
            1,
        ),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::SourcePairIndexInvalid),
        "an inconsistent target registry fails closed instead of indexing absent lineage"
    );
}

#[test]
fn archived_source_pair_index_is_linear_bounded_and_fail_closed() {
    let source = subdivided_cross_geometry_fixture().source_layer_order;
    assert_eq!(
        source.face_pair_orders.len(),
        1,
        "the source diagonal fixture has one certified directed overlap"
    );
    let index = build_source_pair_direction_index_v1(&source, source.face_pair_orders.len())
        .expect("the inclusive pair cap admits the certified source relation");
    let order = source.face_pair_orders[0].clone();
    assert_eq!(
        index.get(&canonical_face_id_pair(
            order.lower_face.face_id,
            order.upper_face.face_id,
        )),
        Some(&RequiredLayerOrderPair {
            lower_face: order.lower_face,
            upper_face: order.upper_face,
        })
    );
    assert_eq!(
        build_source_pair_direction_index_v1(&source, source.face_pair_orders.len() - 1),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::SourcePairResourceLimit)
    );

    let mut duplicate = source.clone();
    duplicate.face_pair_orders.push(order.clone());
    assert_eq!(
        build_source_pair_direction_index_v1(&duplicate, duplicate.face_pair_orders.len()),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::SourcePairIndexInvalid)
    );

    let mut conflicting = source.clone();
    let mut reverse = order.clone();
    std::mem::swap(&mut reverse.lower_face, &mut reverse.upper_face);
    conflicting.face_pair_orders.push(reverse);
    assert_eq!(
        build_source_pair_direction_index_v1(&conflicting, conflicting.face_pair_orders.len()),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::SourcePairIndexInvalid)
    );

    let mut equal = source.clone();
    equal.face_pair_orders[0].upper_face = equal.face_pair_orders[0].lower_face;
    assert_eq!(
        build_source_pair_direction_index_v1(&equal, equal.face_pair_orders.len()),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::SourcePairIndexInvalid)
    );

    let mut unknown = source;
    unknown.face_pair_orders[0].lower_face.face_id = FaceId::new();
    assert_eq!(
        build_source_pair_direction_index_v1(&unknown, unknown.face_pair_orders.len()),
        Err(ArchivedRefinedNonFlatLayerOrderErrorV1::SourcePairIndexInvalid)
    );
}

#[test]
fn topology_builder_rejects_overlapping_carriers_and_exact_limits() {
    let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
    let (source_pattern, source_paper) = sheet.into_parts();
    let start = vertex_position(&source_pattern, source_paper.boundary_vertices[0]);
    let end = vertex_position(&source_pattern, source_paper.boundary_vertices[1]);
    let expected = [ExpectedStackedFoldCreaseV1 {
        start,
        end,
        kind: EdgeKind::Mountain,
    }];
    let carriers = source_pattern.edges.len() + expected.len();
    let pair_tests = carriers * (carriers - 1) / 2;
    let inclusive = StackedFoldTopologyBuildLimitsV1 {
        max_carriers: carriers,
        max_pair_tests: pair_tests,
        ..StackedFoldTopologyBuildLimitsV1::default()
    };
    assert!(matches!(
        build_stacked_fold_topology_v1(
            ProjectId::new(),
            0,
            &source_pattern,
            &source_paper,
            &expected,
            inclusive
        ),
        Err(StackedFoldTopologyBuildErrorV1::CarrierOverlap { .. })
    ));
    assert_eq!(
        build_stacked_fold_topology_v1(
            ProjectId::new(),
            0,
            &source_pattern,
            &source_paper,
            &[],
            StackedFoldTopologyBuildLimitsV1 {
                max_carriers: source_pattern.edges.len() - 1,
                ..StackedFoldTopologyBuildLimitsV1::default()
            }
        ),
        Err(StackedFoldTopologyBuildErrorV1::ResourceLimit {
            resource: StackedFoldTopologyBuildResourceV1::Carriers,
            actual: source_pattern.edges.len(),
            maximum: source_pattern.edges.len() - 1,
        })
    );
    let mut missing_boundary_vertex = source_paper.clone();
    let missing = VertexId::new();
    missing_boundary_vertex.boundary_vertices[0] = missing;
    assert_eq!(
        build_stacked_fold_topology_v1(
            ProjectId::new(),
            0,
            &source_pattern,
            &missing_boundary_vertex,
            &[],
            StackedFoldTopologyBuildLimitsV1::default(),
        ),
        Err(StackedFoldTopologyBuildErrorV1::PaperBoundaryVertexMissing { vertex: missing })
    );
}

fn subdivided_cross_geometry_fixture() -> GeometryFixture {
    let identity = ProjectId::new();
    let source_revision = 12;
    let target_revision = 13;
    let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
    let (mut source_pattern, paper) = sheet.into_parts();
    let source_hinge = EdgeId::new();
    source_pattern.edges.push(Edge {
        id: source_hinge,
        start: paper.boundary_vertices[0],
        end: paper.boundary_vertices[2],
        kind: EdgeKind::Mountain,
    });
    let source_layer_order = proven_layer_order(identity, source_revision, &source_pattern, &paper);

    let mut target_pattern = source_pattern.clone();
    let center = VertexId::new();
    target_pattern.vertices.push(Vertex {
        id: center,
        position: Point2::new(200.0, 200.0),
    });
    target_pattern
        .edges
        .iter_mut()
        .find(|edge| edge.id == source_hinge)
        .expect("source hinge")
        .end = center;
    target_pattern.edges.extend([
        Edge {
            id: EdgeId::new(),
            start: center,
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: EdgeId::new(),
            start: paper.boundary_vertices[1],
            end: center,
            kind: EdgeKind::Valley,
        },
        Edge {
            id: EdgeId::new(),
            start: center,
            end: paper.boundary_vertices[3],
            kind: EdgeKind::Valley,
        },
    ]);
    let expected_creases = vec![ExpectedStackedFoldCreaseV1 {
        start: vertex_position(&source_pattern, paper.boundary_vertices[1]),
        end: vertex_position(&source_pattern, paper.boundary_vertices[3]),
        kind: EdgeKind::Valley,
    }];

    GeometryFixture {
        identity,
        source_revision,
        target_revision,
        source_pattern,
        source_paper: paper.clone(),
        source_layer_order,
        target_pattern,
        target_paper: paper,
        expected_creases,
    }
}

fn two_new_crossing_creases_fixture() -> GeometryFixture {
    let identity = ProjectId::new();
    let source_revision = 20;
    let target_revision = 21;
    let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
    let (source_pattern, paper) = sheet.into_parts();
    let source_layer_order = proven_layer_order(identity, source_revision, &source_pattern, &paper);
    let center = VertexId::new();
    let mut target_pattern = source_pattern.clone();
    target_pattern.vertices.push(Vertex {
        id: center,
        position: Point2::new(200.0, 200.0),
    });
    target_pattern.edges.extend([
        Edge {
            id: EdgeId::new(),
            start: paper.boundary_vertices[0],
            end: center,
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: EdgeId::new(),
            start: center,
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: EdgeId::new(),
            start: paper.boundary_vertices[1],
            end: center,
            kind: EdgeKind::Valley,
        },
        Edge {
            id: EdgeId::new(),
            start: center,
            end: paper.boundary_vertices[3],
            kind: EdgeKind::Valley,
        },
    ]);
    let expected_creases = vec![
        ExpectedStackedFoldCreaseV1 {
            start: vertex_position(&source_pattern, paper.boundary_vertices[0]),
            end: vertex_position(&source_pattern, paper.boundary_vertices[2]),
            kind: EdgeKind::Mountain,
        },
        ExpectedStackedFoldCreaseV1 {
            start: vertex_position(&source_pattern, paper.boundary_vertices[1]),
            end: vertex_position(&source_pattern, paper.boundary_vertices[3]),
            kind: EdgeKind::Valley,
        },
    ];

    GeometryFixture {
        identity,
        source_revision,
        target_revision,
        source_pattern,
        source_paper: paper.clone(),
        source_layer_order,
        target_pattern,
        target_paper: paper,
        expected_creases,
    }
}

fn vertex_position(pattern: &CreasePattern, id: VertexId) -> Point2 {
    pattern
        .vertices
        .iter()
        .find(|vertex| vertex.id == id)
        .expect("fixture vertex")
        .position
}

fn proven_layer_order(
    identity: ProjectId,
    revision: Revision,
    pattern: &CreasePattern,
    paper: &Paper,
) -> LayerOrderSnapshot {
    let source_topology = analyze_faces(FaceExtractionInput {
        identity_namespace: identity,
        source_revision: revision,
        paper,
        pattern,
    })
    .snapshot
    .expect("source topology");
    let local = analyze_local_flat_foldability(paper, pattern);
    let report = analyze_global_flat_foldability(
        GlobalFlatFoldabilityInput::current_with_geometry(
            identity,
            paper,
            pattern,
            &source_topology,
            &local,
        ),
        GlobalFlatFoldabilityLimits::default(),
    )
    .expect("global analysis");
    report.layer_order().expect("possible layer order").clone()
}

#[test]
fn core_non_flat_evidence_keeps_exact_transport_and_resource_behavior() {
    let project = ProjectId::new();
    let sheet = create_rectangular_sheet(100.0, 100.0, false).expect("rectangular sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let hinge = EdgeId::new();
    pattern.edges.push(Edge {
        id: hinge,
        start: paper.boundary_vertices[0],
        end: paper.boundary_vertices[2],
        kind: EdgeKind::Mountain,
    });
    let angles = CanonicalHingeAngles::new(vec![HingeAngle::new(hinge, 90.0).expect("angle")])
        .expect("canonical angle");
    let source_flat = proven_layer_order(project, 1, &pattern, &paper);
    let fixed = source_flat.material_faces[0].face_id;
    let source = revalidate_current_non_flat_layer_order_v1(
        project,
        1,
        &pattern,
        &paper,
        Some(fixed),
        &angles,
        &source_flat,
        1,
    )
    .expect("source non-flat evidence");
    let target_flat = proven_layer_order(project, 2, &pattern, &paper);
    let target = revalidate_current_non_flat_layer_order_v1(
        project,
        2,
        &pattern,
        &paper,
        Some(fixed),
        &angles,
        &target_flat,
        1,
    )
    .expect("target non-flat evidence");
    let proof = ori_collision::certify_non_flat_cell_transport_v1(&source, &target)
        .expect("exact core transport");
    assert!(proof.is_for(&source, &target));
    assert_eq!(proof.target().folded_faces().len(), 2);
    assert!(matches!(
        ori_collision::certify_non_flat_cell_transport_v1(&source, &source),
        Err(ori_collision::NonFlatCellTransportErrorV1::BindingMismatch)
    ));
    assert!(matches!(
        ori_collision::certify_non_flat_cell_transport_with_limits_v1(
            &source,
            &target,
            ori_collision::NonFlatCellTransportLimitsV1 {
                max_faces: 1,
                ..ori_collision::NonFlatCellTransportLimitsV1::default()
            },
        ),
        Err(ori_collision::NonFlatCellTransportErrorV1::ResourceLimit)
    ));
    let different_angles =
        CanonicalHingeAngles::new(vec![HingeAngle::new(hinge, 80.0).expect("angle")])
            .expect("canonical angle");
    let different = revalidate_current_non_flat_layer_order_v1(
        project,
        2,
        &pattern,
        &paper,
        Some(fixed),
        &different_angles,
        &target_flat,
        1,
    )
    .expect("different core evidence");
    assert!(!proof.is_for(&source, &different));
    assert_eq!(
        ori_collision::validate_non_flat_layer_order_structure_v1(&source),
        Ok(())
    );
    assert_eq!(
        ori_collision::validate_non_flat_layer_order_structure_v1(&target),
        Ok(())
    );
    assert_eq!(
        ori_collision::validate_non_flat_layer_order_structure_v1(proof.target()),
        Ok(())
    );
}

#[test]
fn proves_one_source_face_split_into_two_canonical_descendants() {
    let fixture = fixture();
    let lineage = prepare_face_lineage_v1(fixture.input(), FaceLineageLimits::default())
        .expect("prove lineage");

    assert_eq!(lineage.identity_namespace(), fixture.identity);
    assert_eq!(lineage.source_revision(), 7);
    assert_eq!(lineage.target_revision(), 8);
    assert_eq!(
        lineage.source_fingerprint(),
        fold_model_fingerprint_v1(&fixture.source_pattern, &fixture.source_paper)
    );
    assert_eq!(
        lineage.target_fingerprint(),
        fold_model_fingerprint_v1(&fixture.target_pattern, &fixture.target_paper)
    );
    assert_eq!(lineage.records().len(), 1);
    assert_eq!(lineage.records()[0].descendants().len(), 2);
    assert!(
        lineage.records()[0]
            .descendants()
            .windows(2)
            .all(|faces| compare_layer_faces(&faces[0], &faces[1]) == Ordering::Less)
    );
}

#[test]
fn lineage_is_invariant_to_storage_order_and_new_edge_direction() {
    let fixture = fixture();
    let expected = prepare_face_lineage_v1(fixture.input(), FaceLineageLimits::default())
        .expect("baseline lineage");

    let mut reordered = fixture.target_pattern.clone();
    reordered.vertices.reverse();
    reordered.edges.reverse();
    let mut reordered_paper = fixture.target_paper.clone();
    reordered_paper.boundary_vertices.rotate_left(1);
    reordered_paper.boundary_vertices.reverse();
    let fold = reordered
        .edges
        .iter_mut()
        .find(|edge| matches!(edge.kind, EdgeKind::Mountain))
        .expect("new fold");
    std::mem::swap(&mut fold.start, &mut fold.end);
    let input = FaceLineageInput {
        target_pattern: &reordered,
        target_paper: &reordered_paper,
        ..fixture.input()
    };

    assert_eq!(
        prepare_face_lineage_v1(input, FaceLineageLimits::default()),
        Ok(expected)
    );
}

#[test]
fn proves_two_source_faces_each_split_after_shared_hinge_subdivision() {
    let identity = ProjectId::new();
    let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
    let (mut source_pattern, paper) = sheet.into_parts();
    let source_hinge = EdgeId::new();
    source_pattern.edges.push(Edge {
        id: source_hinge,
        start: paper.boundary_vertices[0],
        end: paper.boundary_vertices[2],
        kind: EdgeKind::Mountain,
    });
    let source_layer_order = proven_layer_order(identity, 12, &source_pattern, &paper);

    let mut target_pattern = source_pattern.clone();
    let center = VertexId::new();
    target_pattern.vertices.push(Vertex {
        id: center,
        position: Point2::new(200.0, 200.0),
    });
    target_pattern
        .edges
        .iter_mut()
        .find(|edge| edge.id == source_hinge)
        .expect("source hinge")
        .end = center;
    target_pattern.edges.extend([
        Edge {
            id: EdgeId::new(),
            start: center,
            end: paper.boundary_vertices[2],
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: EdgeId::new(),
            start: paper.boundary_vertices[1],
            end: center,
            kind: EdgeKind::Valley,
        },
        Edge {
            id: EdgeId::new(),
            start: center,
            end: paper.boundary_vertices[3],
            kind: EdgeKind::Valley,
        },
    ]);

    let lineage = prepare_face_lineage_v1(
        FaceLineageInput {
            identity_namespace: identity,
            source_revision: 12,
            source_paper: &paper,
            source_pattern: &source_pattern,
            source_layer_order: &source_layer_order,
            target_revision: 13,
            target_paper: &paper,
            target_pattern: &target_pattern,
        },
        FaceLineageLimits::default(),
    )
    .expect("prove two-face lineage");

    assert_eq!(lineage.records().len(), 2);
    assert!(
        lineage
            .records()
            .iter()
            .all(|record| record.descendants().len() == 2)
    );
    let descendant_ids = lineage
        .records()
        .iter()
        .flat_map(FaceLineageRecord::descendants)
        .map(|face| face.face_id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(descendant_ids.len(), 4);
}

#[test]
fn geometry_proof_accepts_only_the_explicit_mountain_delta() {
    let fixture = simple_geometry_fixture();
    let lineage = fixture.lineage();
    let proof = prepare_stacked_fold_geometry_v1(
        fixture.geometry_input(&lineage),
        StackedFoldGeometryLimitsV1::default(),
    )
    .expect("prove simple stacked-fold geometry");

    assert_eq!(proof.lineage(), &lineage);
    assert_eq!(
        proof.source_edges().len(),
        fixture.source_pattern.edges.len()
    );
    assert!(
        proof
            .source_edges()
            .iter()
            .all(|subdivision| subdivision.target_edges().len() == 1)
    );
    assert_eq!(proof.expected_creases().len(), 1);
    assert_eq!(proof.expected_creases()[0].start(), Point2::new(0.0, 0.0));
    assert_eq!(proof.expected_creases()[0].end(), Point2::new(400.0, 400.0));
    assert_eq!(proof.expected_creases()[0].kind(), EdgeKind::Mountain);
    assert_eq!(proof.expected_creases()[0].target_edges().len(), 1);
}

#[test]
fn geometry_proof_accepts_exact_source_and_expected_subdivisions() {
    let fixture = subdivided_cross_geometry_fixture();
    let source_hinge = fixture
        .source_pattern
        .edges
        .iter()
        .find(|edge| edge.kind == EdgeKind::Mountain)
        .expect("source hinge")
        .id;
    let lineage = fixture.lineage();
    let proof = prepare_stacked_fold_geometry_v1(
        fixture.geometry_input(&lineage),
        StackedFoldGeometryLimitsV1::default(),
    )
    .expect("prove subdivided source and expected crease");

    let source_subdivision = proof
        .source_edges()
        .iter()
        .find(|subdivision| subdivision.source_edge() == source_hinge)
        .expect("source hinge subdivision");
    assert_eq!(source_subdivision.target_edges().len(), 2);
    assert_eq!(proof.expected_creases().len(), 1);
    assert_eq!(proof.expected_creases()[0].kind(), EdgeKind::Valley);
    assert_eq!(proof.expected_creases()[0].target_edges().len(), 2);
}

#[test]
fn geometry_proof_is_invariant_to_storage_expected_order_and_edge_direction() {
    let fixture = two_new_crossing_creases_fixture();
    let lineage = fixture.lineage();
    let expected = prepare_stacked_fold_geometry_v1(
        fixture.geometry_input(&lineage),
        StackedFoldGeometryLimitsV1::default(),
    )
    .expect("baseline geometry proof");

    let mut source_pattern = fixture.source_pattern.clone();
    source_pattern.vertices.reverse();
    source_pattern.edges.reverse();
    for edge in &mut source_pattern.edges {
        std::mem::swap(&mut edge.start, &mut edge.end);
    }
    let mut target_pattern = fixture.target_pattern.clone();
    target_pattern.vertices.reverse();
    target_pattern.edges.reverse();
    for edge in &mut target_pattern.edges {
        std::mem::swap(&mut edge.start, &mut edge.end);
    }
    let mut source_paper = fixture.source_paper.clone();
    source_paper.boundary_vertices.rotate_left(1);
    source_paper.boundary_vertices.reverse();
    let mut target_paper = fixture.target_paper.clone();
    target_paper.boundary_vertices.rotate_right(1);
    target_paper.boundary_vertices.reverse();
    let mut expected_creases = fixture.expected_creases.clone();
    expected_creases.reverse();
    for crease in &mut expected_creases {
        std::mem::swap(&mut crease.start, &mut crease.end);
    }
    let reordered_input = StackedFoldGeometryInputV1 {
        identity_namespace: fixture.identity,
        source_revision: fixture.source_revision,
        source_paper: &source_paper,
        source_pattern: &source_pattern,
        target_revision: fixture.target_revision,
        target_paper: &target_paper,
        target_pattern: &target_pattern,
        face_lineage: &lineage,
        expected_creases: &expected_creases,
    };

    assert_eq!(
        prepare_stacked_fold_geometry_v1(reordered_input, StackedFoldGeometryLimitsV1::default()),
        Ok(expected)
    );
}

#[test]
fn geometry_proof_rebinds_identity_revisions_and_both_fingerprints() {
    let fixture = simple_geometry_fixture();
    let lineage = fixture.lineage();
    let input = fixture.geometry_input(&lineage);

    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                identity_namespace: ProjectId::new(),
                ..input
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::LineageIdentityMismatch)
    );
    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                target_revision: fixture.target_revision + 1,
                ..input
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::LineageRevisionMismatch)
    );

    let mut changed_source = fixture.source_pattern.clone();
    changed_source.vertices[0].position.x = 1.0;
    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                source_pattern: &changed_source,
                ..input
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::LineageSourceFingerprintMismatch)
    );

    let mut changed_target = fixture.target_pattern.clone();
    changed_target.vertices.push(Vertex {
        id: VertexId::new(),
        position: Point2::new(123.0, 234.0),
    });
    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                target_pattern: &changed_target,
                ..input
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::LineageTargetFingerprintMismatch)
    );
}

#[test]
fn expected_crease_input_is_nonempty_finite_nondegenerate_and_mv_only() {
    let fixture = simple_geometry_fixture();
    let lineage = fixture.lineage();
    let input = fixture.geometry_input(&lineage);

    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                expected_creases: &[],
                ..input
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::ExpectedCreaseSetEmpty)
    );

    let non_finite = [ExpectedStackedFoldCreaseV1 {
        start: Point2::new(f64::NAN, 0.0),
        ..fixture.expected_creases[0]
    }];
    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                expected_creases: &non_finite,
                ..input
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::ExpectedCreaseNonFinite)
    );

    let degenerate = [ExpectedStackedFoldCreaseV1 {
        end: fixture.expected_creases[0].start,
        ..fixture.expected_creases[0]
    }];
    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                expected_creases: &degenerate,
                ..input
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::ExpectedCreaseDegenerate)
    );

    for kind in [EdgeKind::Auxiliary, EdgeKind::Boundary, EdgeKind::Cut] {
        let unsupported = [ExpectedStackedFoldCreaseV1 {
            kind,
            ..fixture.expected_creases[0]
        }];
        assert_eq!(
            prepare_stacked_fold_geometry_v1(
                StackedFoldGeometryInputV1 {
                    expected_creases: &unsupported,
                    ..input
                },
                StackedFoldGeometryLimitsV1::default(),
            ),
            Err(StackedFoldGeometryErrorV1::ExpectedCreaseKindUnsupported)
        );
    }
}

#[test]
fn coincident_expected_and_source_carriers_are_rejected() {
    let fixture = simple_geometry_fixture();
    let lineage = fixture.lineage();
    let input = fixture.geometry_input(&lineage);
    let duplicate = [
        fixture.expected_creases[0],
        ExpectedStackedFoldCreaseV1 {
            start: fixture.expected_creases[0].end,
            end: fixture.expected_creases[0].start,
            kind: EdgeKind::Mountain,
        },
    ];
    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                expected_creases: &duplicate,
                ..input
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::ExpectedCreasesOverlap {
            first: 0,
            second: 1,
        })
    );

    let boundary_start = fixture.source_paper.boundary_vertices[0];
    let boundary_end = fixture.source_paper.boundary_vertices[1];
    let boundary_edge = fixture
        .source_pattern
        .edges
        .iter()
        .find(|edge| {
            (edge.start == boundary_start && edge.end == boundary_end)
                || (edge.start == boundary_end && edge.end == boundary_start)
        })
        .expect("source boundary edge")
        .id;
    let overlaps_source = [ExpectedStackedFoldCreaseV1 {
        start: vertex_position(&fixture.source_pattern, boundary_start),
        end: vertex_position(&fixture.source_pattern, boundary_end),
        kind: EdgeKind::Mountain,
    }];
    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                expected_creases: &overlaps_source,
                ..input
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(
            StackedFoldGeometryErrorV1::ExpectedCreaseOverlapsSourceEdge {
                expected_index: 0,
                source_edge: boundary_edge,
            }
        )
    );
}

#[test]
fn wrong_missing_and_extra_expected_creases_are_rejected_exactly() {
    let fixture = simple_geometry_fixture();
    let lineage = fixture.lineage();
    let input = fixture.geometry_input(&lineage);
    let target_fold = fixture
        .target_pattern
        .edges
        .iter()
        .find(|edge| edge.kind == EdgeKind::Mountain)
        .expect("target fold")
        .id;
    let wrong_kind = [ExpectedStackedFoldCreaseV1 {
        kind: EdgeKind::Valley,
        ..fixture.expected_creases[0]
    }];
    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                expected_creases: &wrong_kind,
                ..input
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::TargetEdgeWithoutCarrier { edge: target_fold })
    );

    let missing_target = [
        fixture.expected_creases[0],
        ExpectedStackedFoldCreaseV1 {
            start: Point2::new(0.0, 400.0),
            end: Point2::new(400.0, 0.0),
            kind: EdgeKind::Valley,
        },
    ];
    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                expected_creases: &missing_target,
                ..input
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::CarrierCoverageMismatch {
            carrier: StackedFoldGeometryCarrierV1::ExpectedCrease(1),
        })
    );

    let two_creases = two_new_crossing_creases_fixture();
    let two_lineage = two_creases.lineage();
    let only_mountain = [two_creases.expected_creases[0]];
    assert!(matches!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                expected_creases: &only_mountain,
                ..two_creases.geometry_input(&two_lineage)
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::TargetEdgeWithoutCarrier { edge })
            if two_creases
                .target_pattern
                .edges
                .iter()
                .any(|candidate| candidate.id == edge && candidate.kind == EdgeKind::Valley)
    ));
}

#[test]
fn source_edge_identity_and_kind_cannot_change_during_subdivision() {
    let fixture = subdivided_cross_geometry_fixture();
    let source_hinge = fixture
        .source_pattern
        .edges
        .iter()
        .find(|edge| edge.kind == EdgeKind::Mountain)
        .expect("source hinge")
        .id;

    let mut changed_kind = fixture.clone();
    changed_kind
        .target_pattern
        .edges
        .iter_mut()
        .find(|edge| edge.id == source_hinge)
        .expect("target source edge")
        .kind = EdgeKind::Valley;
    let changed_kind_lineage = changed_kind.lineage();
    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            changed_kind.geometry_input(&changed_kind_lineage),
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::SourceEdgeKindChanged { edge: source_hinge })
    );

    let mut changed_identity = fixture;
    changed_identity
        .target_pattern
        .edges
        .iter_mut()
        .find(|edge| edge.id == source_hinge)
        .expect("target source edge")
        .id = EdgeId::new();
    let changed_identity_lineage = changed_identity.lineage();
    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            changed_identity.geometry_input(&changed_identity_lineage),
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::SourceEdgeIdentityMissing { edge: source_hinge })
    );
}

#[test]
fn new_unrelated_target_vertices_are_rejected_even_with_valid_lineage() {
    let mut fixture = simple_geometry_fixture();
    let isolated = VertexId::new();
    fixture.target_pattern.vertices.push(Vertex {
        id: isolated,
        position: Point2::new(123.0, 234.0),
    });
    let lineage = fixture.lineage();

    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            fixture.geometry_input(&lineage),
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::NewTargetVertexIsolated { vertex: isolated })
    );
}

#[test]
fn moving_an_existing_unrelated_vertex_is_rejected_even_with_valid_lineage() {
    let identity = ProjectId::new();
    let sheet = create_rectangular_sheet(400.0, 400.0, false).expect("create rectangle");
    let (mut source_pattern, paper) = sheet.into_parts();
    let isolated = VertexId::new();
    source_pattern.vertices.push(Vertex {
        id: isolated,
        position: Point2::new(500.0, 500.0),
    });
    let source_layer_order = proven_layer_order(identity, 30, &source_pattern, &paper);
    let mut target_pattern = source_pattern.clone();
    target_pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == isolated)
        .expect("isolated source vertex")
        .position = Point2::new(501.0, 500.0);
    target_pattern.edges.push(Edge {
        id: EdgeId::new(),
        start: paper.boundary_vertices[0],
        end: paper.boundary_vertices[2],
        kind: EdgeKind::Mountain,
    });
    let expected_creases = [ExpectedStackedFoldCreaseV1 {
        start: vertex_position(&source_pattern, paper.boundary_vertices[0]),
        end: vertex_position(&source_pattern, paper.boundary_vertices[2]),
        kind: EdgeKind::Mountain,
    }];
    let lineage = prepare_face_lineage_v1(
        FaceLineageInput {
            identity_namespace: identity,
            source_revision: 30,
            source_paper: &paper,
            source_pattern: &source_pattern,
            source_layer_order: &source_layer_order,
            target_revision: 31,
            target_paper: &paper,
            target_pattern: &target_pattern,
        },
        FaceLineageLimits::default(),
    )
    .expect("lineage intentionally ignores isolated draft movement");

    assert_eq!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                identity_namespace: identity,
                source_revision: 30,
                source_paper: &paper,
                source_pattern: &source_pattern,
                target_revision: 31,
                target_paper: &paper,
                target_pattern: &target_pattern,
                face_lineage: &lineage,
                expected_creases: &expected_creases,
            },
            StackedFoldGeometryLimitsV1::default(),
        ),
        Err(StackedFoldGeometryErrorV1::SourceVertexMoved { vertex: isolated })
    );
}

#[test]
fn geometry_resource_limits_admit_equality_and_reject_one_less() {
    let fixture = simple_geometry_fixture();
    let lineage = fixture.lineage();
    let exact = StackedFoldGeometryLimitsV1 {
        max_source_vertices: 4,
        max_source_edges: 4,
        max_source_paper_boundary_vertices: 4,
        max_target_vertices: 4,
        max_target_edges: 5,
        max_target_paper_boundary_vertices: 4,
        max_expected_creases: 1,
        max_lineage_records: 1,
        max_lineage_descendants: 2,
        max_edge_carrier_tests: 25,
        max_carrier_overlap_tests: 4,
    };
    prepare_stacked_fold_geometry_v1(fixture.geometry_input(&lineage), exact)
        .expect("all documented limits admit equality");

    for (limits, resource, actual, maximum) in [
        (
            StackedFoldGeometryLimitsV1 {
                max_edge_carrier_tests: 24,
                ..exact
            },
            StackedFoldGeometryResourceV1::EdgeCarrierTests,
            25,
            24,
        ),
        (
            StackedFoldGeometryLimitsV1 {
                max_carrier_overlap_tests: 3,
                ..exact
            },
            StackedFoldGeometryResourceV1::CarrierOverlapTests,
            4,
            3,
        ),
        (
            StackedFoldGeometryLimitsV1 {
                max_lineage_descendants: 1,
                ..exact
            },
            StackedFoldGeometryResourceV1::LineageDescendants,
            2,
            1,
        ),
        (
            StackedFoldGeometryLimitsV1 {
                max_expected_creases: 0,
                ..exact
            },
            StackedFoldGeometryResourceV1::ExpectedCreases,
            1,
            0,
        ),
    ] {
        assert_eq!(
            prepare_stacked_fold_geometry_v1(fixture.geometry_input(&lineage), limits),
            Err(StackedFoldGeometryErrorV1::ResourceLimit {
                resource,
                actual,
                maximum,
            })
        );
    }
}

#[test]
fn geometry_failure_is_pure_and_leaves_editor_state_unchanged() {
    let fixture = simple_geometry_fixture();
    let lineage = fixture.lineage();
    let editor =
        EditorState::with_paper(fixture.source_pattern.clone(), fixture.source_paper.clone());
    let before_pattern = editor.pattern().clone();
    let before_paper = editor.paper().clone();
    let before_timeline = editor.instruction_timeline().clone();
    let before_revision = editor.revision();
    let before_undo = editor.can_undo();
    let before_redo = editor.can_redo();
    let wrong_kind = [ExpectedStackedFoldCreaseV1 {
        kind: EdgeKind::Valley,
        ..fixture.expected_creases[0]
    }];

    assert!(
        prepare_stacked_fold_geometry_v1(
            StackedFoldGeometryInputV1 {
                expected_creases: &wrong_kind,
                ..fixture.geometry_input(&lineage)
            },
            StackedFoldGeometryLimitsV1::default(),
        )
        .is_err()
    );
    assert_eq!(editor.pattern(), &before_pattern);
    assert_eq!(editor.paper(), &before_paper);
    assert_eq!(editor.instruction_timeline(), &before_timeline);
    assert_eq!(editor.revision(), before_revision);
    assert_eq!(editor.can_undo(), before_undo);
    assert_eq!(editor.can_redo(), before_redo);
}

#[test]
fn exact_carrier_coverage_distinguishes_adjacency_gap_and_overlap() {
    let carrier = GeometryCarrier {
        public: StackedFoldGeometryCarrierV1::ExpectedCrease(0),
        start: Point2::new(0.0, 0.0),
        end: Point2::new(3.0, 0.0),
        kind: EdgeKind::Mountain,
    };
    let edge = |start: f64, end: f64| GeometryEdgeRecord {
        id: EdgeId::new(),
        start_vertex: VertexId::new(),
        end_vertex: VertexId::new(),
        start: Point2::new(start, 0.0),
        end: Point2::new(end, 0.0),
        kind: EdgeKind::Mountain,
    };

    assert!(carrier_has_exact_coverage(
        carrier,
        &[edge(3.0, 2.0), edge(0.0, 1.0), edge(2.0, 1.0)]
    ));
    assert!(!carrier_has_exact_coverage(
        carrier,
        &[edge(0.0, 1.0), edge(2.0, 3.0)]
    ));
    assert!(!carrier_has_exact_coverage(
        carrier,
        &[edge(0.0, 2.0), edge(1.0, 3.0)]
    ));
}

#[test]
fn exact_overlap_rejects_positive_interval_but_allows_point_contact_and_crossing() {
    let horizontal_start = Point2::new(0.0, 0.0);
    let horizontal_end = Point2::new(2.0, 0.0);
    assert!(
        segments_share_positive_collinear_interval(
            horizontal_start,
            horizontal_end,
            Point2::new(3.0, 0.0),
            Point2::new(1.0, 0.0),
        )
        .unwrap()
    );
    assert!(
        !segments_share_positive_collinear_interval(
            horizontal_start,
            horizontal_end,
            Point2::new(2.0, 0.0),
            Point2::new(3.0, 0.0),
        )
        .unwrap()
    );
    assert!(
        !segments_share_positive_collinear_interval(
            horizontal_start,
            horizontal_end,
            Point2::new(1.0, -1.0),
            Point2::new(1.0, 1.0),
        )
        .unwrap()
    );
}

#[test]
fn stale_layer_order_is_rejected_before_any_lineage_is_published() {
    let mut fixture = fixture();
    fixture.source_layer_order.provenance.source.source_revision = 6;

    assert_eq!(
        prepare_face_lineage_v1(fixture.input(), FaceLineageLimits::default()),
        Err(FaceLineageError::LayerOrderNotCurrent)
    );
}

#[test]
fn oversized_layer_registry_is_rejected_without_clone_or_target_work() {
    let mut fixture = fixture();
    let repeated_face = fixture.source_layer_order.material_faces[0];
    fixture
        .source_layer_order
        .material_faces
        .resize(DEFAULT_MAX_FACE_LINEAGE_SOURCE_FACES + 1, repeated_face);
    fixture.target_pattern.edges[0].start = VertexId::new();

    assert_eq!(
        prepare_face_lineage_v1(fixture.input(), FaceLineageLimits::default()),
        Err(FaceLineageError::LayerOrderMaterialRegistryMismatch)
    );
}

#[test]
fn revision_gap_and_unrelated_paper_changes_are_rejected() {
    let fixture = fixture();
    let revision_gap = FaceLineageInput {
        target_revision: 9,
        ..fixture.input()
    };
    assert_eq!(
        prepare_face_lineage_v1(revision_gap, FaceLineageLimits::default()),
        Err(FaceLineageError::TargetRevisionNotNext {
            expected: 8,
            actual: 9,
        })
    );

    let mut changed_paper = fixture.target_paper.clone();
    changed_paper.front.color.red ^= 1;
    let paper_change = FaceLineageInput {
        target_paper: &changed_paper,
        ..fixture.input()
    };
    assert_eq!(
        prepare_face_lineage_v1(paper_change, FaceLineageLimits::default()),
        Err(FaceLineageError::PaperPropertiesChanged)
    );

    let mut changed_display_unit = fixture.target_paper.clone();
    changed_display_unit.length_display_unit = LengthDisplayUnit::Centimeter;
    let display_unit_change = FaceLineageInput {
        target_paper: &changed_display_unit,
        ..fixture.input()
    };
    assert_eq!(
        prepare_face_lineage_v1(display_unit_change, FaceLineageLimits::default()),
        Err(FaceLineageError::PaperPropertiesChanged)
    );
}

#[test]
fn exact_per_source_area_rejects_material_loss() {
    let fixture = fixture();
    let smaller = create_rectangular_sheet(200.0, 200.0, false).expect("smaller rectangle");
    let (mut target_pattern, target_paper) = smaller.into_parts();
    target_pattern.edges.push(Edge {
        id: EdgeId::new(),
        start: target_paper.boundary_vertices[0],
        end: target_paper.boundary_vertices[2],
        kind: EdgeKind::Mountain,
    });
    let input = FaceLineageInput {
        target_pattern: &target_pattern,
        target_paper: &target_paper,
        ..fixture.input()
    };

    assert!(matches!(
        prepare_face_lineage_v1(input, FaceLineageLimits::default()),
        Err(FaceLineageError::SourceFaceAreaMismatch { .. })
    ));
}

#[test]
fn no_geometry_split_is_not_a_stacked_fold_lineage() {
    let fixture = fixture();
    let input = FaceLineageInput {
        target_pattern: &fixture.source_pattern,
        target_paper: &fixture.source_paper,
        ..fixture.input()
    };

    assert_eq!(
        prepare_face_lineage_v1(input, FaceLineageLimits::default()),
        Err(FaceLineageError::NoSourceFaceSplit)
    );
}

#[test]
fn stale_revision_and_resource_failure_leave_editor_state_unchanged() {
    let fixture = fixture();
    let editor =
        EditorState::with_paper(fixture.source_pattern.clone(), fixture.source_paper.clone());
    let before_pattern = editor.pattern().clone();
    let before_paper = editor.paper().clone();
    let before_timeline = editor.instruction_timeline().clone();
    let before_revision = editor.revision();
    let before_undo = editor.can_undo();
    let before_redo = editor.can_redo();

    let stale = FaceLineageInput {
        source_revision: u64::MAX,
        target_revision: 0,
        ..fixture.input()
    };
    assert_eq!(
        prepare_face_lineage_v1(stale, FaceLineageLimits::default()),
        Err(FaceLineageError::SourceRevisionCannotAdvance)
    );

    let limits = FaceLineageLimits {
        max_target_edges: fixture.target_pattern.edges.len() - 1,
        ..FaceLineageLimits::default()
    };
    assert!(matches!(
        prepare_face_lineage_v1(fixture.input(), limits),
        Err(FaceLineageError::ResourceLimit {
            resource: FaceLineageResource::TargetEdges,
            ..
        })
    ));

    assert_eq!(editor.pattern(), &before_pattern);
    assert_eq!(editor.paper(), &before_paper);
    assert_eq!(editor.instruction_timeline(), &before_timeline);
    assert_eq!(editor.revision(), before_revision);
    assert_eq!(editor.can_undo(), before_undo);
    assert_eq!(editor.can_redo(), before_redo);
}

#[test]
fn face_lineage_rejects_json_revision_ceiling_and_larger_source_revisions() {
    let fixture = fixture();
    let final_source_revision = crate::MAX_REVISION - 1;
    let final_source_layer_order = proven_layer_order(
        fixture.identity,
        final_source_revision,
        &fixture.source_pattern,
        &fixture.source_paper,
    );
    let final_valid_input = FaceLineageInput {
        source_revision: final_source_revision,
        source_layer_order: &final_source_layer_order,
        target_revision: crate::MAX_REVISION,
        ..fixture.input()
    };
    let final_lineage = prepare_face_lineage_v1(final_valid_input, FaceLineageLimits::default())
        .expect("the final JSON-safe target revision must remain admissible");
    assert_eq!(final_lineage.source_revision(), final_source_revision);
    assert_eq!(final_lineage.target_revision(), crate::MAX_REVISION);

    for source_revision in [crate::MAX_REVISION, crate::MAX_REVISION + 1, u64::MAX] {
        let input = FaceLineageInput {
            source_revision,
            target_revision: source_revision.saturating_add(1),
            ..fixture.input()
        };

        assert_eq!(
            prepare_face_lineage_v1(input, FaceLineageLimits::default()),
            Err(FaceLineageError::SourceRevisionCannotAdvance),
            "source revision {source_revision} must not produce a lineage"
        );
    }
}

#[test]
fn exact_work_limits_admit_equality_and_reject_the_next_operation() {
    let fixture = fixture();
    let exact_limit = 2 * 4 * 3 * 2;
    let inclusive = FaceLineageLimits {
        max_face_pairs: 2,
        max_exact_containment_tests: exact_limit,
        ..FaceLineageLimits::default()
    };
    prepare_face_lineage_v1(fixture.input(), inclusive)
        .expect("the documented resource limits admit equality");

    let pair_limited = FaceLineageLimits {
        max_face_pairs: 1,
        ..inclusive
    };
    assert_eq!(
        prepare_face_lineage_v1(fixture.input(), pair_limited),
        Err(FaceLineageError::ResourceLimit {
            resource: FaceLineageResource::FacePairs,
            actual: 2,
            maximum: 1,
        })
    );

    let predicate_limited = FaceLineageLimits {
        max_exact_containment_tests: exact_limit - 1,
        ..inclusive
    };
    assert_eq!(
        prepare_face_lineage_v1(fixture.input(), predicate_limited),
        Err(FaceLineageError::ResourceLimit {
            resource: FaceLineageResource::ExactContainmentTests,
            actual: exact_limit,
            maximum: exact_limit - 1,
        })
    );
}

#[test]
fn convex_vertex_certificate_contains_whole_target_edges() {
    let source = [
        Point2::new(0.0, 0.0),
        Point2::new(10.0, 0.0),
        Point2::new(10.0, 10.0),
        Point2::new(0.0, 10.0),
    ];
    let boundary_chord = [
        Point2::new(0.0, 0.0),
        Point2::new(10.0, 10.0),
        Point2::new(0.0, 10.0),
    ];
    let concave_target = [
        Point2::new(1.0, 1.0),
        Point2::new(9.0, 1.0),
        Point2::new(5.0, 5.0),
        Point2::new(9.0, 9.0),
        Point2::new(1.0, 9.0),
    ];
    let outside = [
        Point2::new(1.0, 1.0),
        Point2::new(11.0, 5.0),
        Point2::new(1.0, 9.0),
    ];

    assert!(polygon_is_within_convex_source(&boundary_chord, &source).unwrap());
    assert!(polygon_is_within_convex_source(&concave_target, &source).unwrap());
    assert!(!polygon_is_within_convex_source(&outside, &source).unwrap());
}

#[test]
fn exact_binary64_units_cover_subnormal_normal_and_maximum_values() {
    let minimum_subnormal = f64::from_bits(1);
    assert_eq!(
        exact_f64_at_minimum_scale(minimum_subnormal),
        BigInt::from(1_u8)
    );
    assert_eq!(
        exact_f64_at_minimum_scale(-minimum_subnormal),
        BigInt::from(-1_i8)
    );
    assert_eq!(
        exact_f64_at_minimum_scale(f64::MIN_POSITIVE),
        BigInt::from(1_u8) << 52_usize
    );
    assert_eq!(
        exact_f64_at_minimum_scale(1.0),
        BigInt::from(1_u8) << 1074_usize
    );
    assert_eq!(
        exact_f64_at_minimum_scale(f64::MAX),
        BigInt::from((1_u64 << 53) - 1) << 2045_usize
    );
    assert_eq!(exact_f64_at_minimum_scale(-0.0), BigInt::from(0_u8));
}

#[test]
fn exact_area_uses_binary64_values_without_rounding_the_sum() {
    let huge = f64::from_bits(0x7fe0_0000_0000_0000);
    let tiny = f64::from_bits(1);
    let polygon = [
        Point2::new(0.0, 0.0),
        Point2::new(huge, 0.0),
        Point2::new(huge, tiny),
        Point2::new(0.0, tiny),
    ];
    assert!(exact_polygon_double_area(&polygon) > BigInt::from(0_u8));
    let mut reversed = polygon;
    reversed.reverse();
    assert_eq!(
        exact_polygon_double_area(&reversed),
        -exact_polygon_double_area(&polygon)
    );
}
