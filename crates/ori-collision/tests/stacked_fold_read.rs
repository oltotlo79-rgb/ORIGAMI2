use ori_collision::{
    FlatEndpointLayerOrderAnchorErrorV1, FlatEndpointLayerOrderInputV1,
    FlatEndpointLayerOrderLimitsV1, FlatEndpointLayerOrderResourceV1, NativeStackedFoldReadGuardV1,
    STACKED_FOLD_MATERIAL_MAP_MODEL_ID_V1, STACKED_FOLD_READ_GUARD_MODEL_ID_V1,
    STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1, StackedFoldFixedSideV1, StackedFoldLinearCandidateV1,
    StackedFoldMaterialMapErrorV1, StackedFoldMaterialMapLimitsV1, StackedFoldReadBindingV1,
    StackedFoldReadErrorV1, StackedFoldReadFailureClassV1, StackedFoldReadLimitsV1,
    StackedFoldReadResourceV1, StackedFoldReadSupportV1, StackedFoldRotationDirectionV1,
    capture_stacked_fold_read_guard_v1, propose_linear_stacked_fold_read_v1,
    revalidate_linear_stacked_fold_read_proposal_v1, revalidate_stacked_fold_read_guard_v1,
    reverse_map_linear_stacked_fold_material_v1,
};
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_foldability::{
    GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, LayerOrderSnapshot,
    analyze_global_flat_foldability,
};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, MaterialTreePose, Point3,
    TreeKinematicsLimits,
};
use ori_topology::{
    FaceExtractionInput, TopologySnapshot, analyze_faces, analyze_local_flat_foldability,
};
use serde::de::DeserializeOwned;

const REVISION: u64 = 17;
const POSE_GENERATION: u64 = 23;
const LAYER_GENERATION: u64 = 29;

struct Fixture {
    project_instance: ProjectId,
    project: ProjectId,
    paper: Paper,
    pattern: CreasePattern,
    topology: TopologySnapshot,
    layer_order: LayerOrderSnapshot,
    model: MaterialTreeKinematicsModel,
}

fn fixed_id<T: DeserializeOwned>(suffix: u64) -> T {
    serde_json::from_str(&format!("\"30000000-0000-4000-8000-{suffix:012x}\""))
        .expect("fixed UUID-backed ID")
}

fn build_fixture(positions: Vec<Point2>, creases: &[(usize, usize, EdgeKind)]) -> Fixture {
    let project_instance = fixed_id(1);
    let project = fixed_id(2);
    let vertices = positions
        .into_iter()
        .enumerate()
        .map(|(index, position)| Vertex {
            id: fixed_id::<VertexId>(0x100 + index as u64),
            position,
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..vertices.len())
        .map(|index| Edge {
            id: fixed_id::<EdgeId>(0x200 + index as u64),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend(
        creases
            .iter()
            .enumerate()
            .map(|(index, (start, end, kind))| Edge {
                id: fixed_id(0x300 + index as u64),
                start: boundary[*start],
                end: boundary[*end],
                kind: *kind,
            }),
    );
    let paper = Paper {
        boundary_vertices: boundary,
        thickness_mm: 0.1,
        ..Paper::default()
    };
    let pattern = CreasePattern { vertices, edges };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: project,
        source_revision: REVISION,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    let topology = report.snapshot.expect("material topology");
    let local = analyze_local_flat_foldability(&paper, &pattern);
    let foldability = analyze_global_flat_foldability(
        GlobalFlatFoldabilityInput::current_with_geometry(
            project, &paper, &pattern, &topology, &local,
        ),
        GlobalFlatFoldabilityLimits::default(),
    )
    .expect("flat-foldability analysis");
    let layer_order = foldability
        .layer_order()
        .expect("certified layer order")
        .clone();
    let model = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("material tree model");
    Fixture {
        project_instance,
        project,
        paper,
        pattern,
        topology,
        layer_order,
        model,
    }
}

fn fixture(with_hinge: bool) -> Fixture {
    let positions = if with_hinge {
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(200.0, 0.0),
            Point2::new(400.0, 0.0),
            Point2::new(400.0, 400.0),
            Point2::new(200.0, 400.0),
            Point2::new(0.0, 400.0),
        ]
    } else {
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
            Point2::new(400.0, 400.0),
            Point2::new(0.0, 400.0),
        ]
    };
    if with_hinge {
        build_fixture(positions, &[(1, 4, EdgeKind::Mountain)])
    } else {
        build_fixture(positions, &[])
    }
}

fn off_center_fixture() -> Fixture {
    build_fixture(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(150.0, 0.0),
            Point2::new(400.0, 0.0),
            Point2::new(400.0, 400.0),
            Point2::new(150.0, 400.0),
            Point2::new(0.0, 400.0),
        ],
        &[(1, 4, EdgeKind::Mountain)],
    )
}

fn three_panel_fixture() -> Fixture {
    build_fixture(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(200.0, 0.0),
            Point2::new(400.0, 0.0),
            Point2::new(600.0, 0.0),
            Point2::new(600.0, 200.0),
            Point2::new(400.0, 200.0),
            Point2::new(200.0, 200.0),
            Point2::new(0.0, 200.0),
        ],
        &[(1, 6, EdgeKind::Mountain), (2, 5, EdgeKind::Valley)],
    )
}

fn binding(fixture: &Fixture) -> StackedFoldReadBindingV1 {
    StackedFoldReadBindingV1::new(
        fixture.project_instance,
        fixture.project,
        REVISION,
        POSE_GENERATION,
        LAYER_GENERATION,
    )
}

fn pose(fixture: &Fixture, angle: f64, root: Option<FaceId>) -> MaterialTreePose {
    let angles = fixture
        .model
        .hinges()
        .iter()
        .map(|hinge| HingeAngle::new(hinge.edge(), angle).expect("valid angle"))
        .collect::<Vec<_>>();
    let angles = CanonicalHingeAngles::new(angles).expect("canonical angles");
    fixture.model.solve(root, &angles).expect("material pose")
}

fn admitted_pose(fixture: &Fixture) -> MaterialTreePose {
    if fixture.model.hinges().is_empty() {
        pose(fixture, 0.0, None)
    } else {
        pose(
            fixture,
            180.0,
            Some(
                fixture
                    .layer_order
                    .reference_face
                    .expect("flat reference face")
                    .face_id,
            ),
        )
    }
}

fn input<'source, 'snapshot>(
    fixture: &'source Fixture,
    pose: &'source MaterialTreePose,
    snapshot: &'snapshot LayerOrderSnapshot,
) -> FlatEndpointLayerOrderInputV1<'source, 'snapshot> {
    FlatEndpointLayerOrderInputV1 {
        identity_namespace: fixture.project,
        source_revision: REVISION,
        paper: &fixture.paper,
        pattern: &fixture.pattern,
        model: &fixture.model,
        pose,
        layer_order: snapshot,
    }
}

fn crossing_candidate(x: f64) -> StackedFoldLinearCandidateV1 {
    StackedFoldLinearCandidateV1::new(
        Point3::new(x, 0.0, 0.0).expect("finite first point"),
        Point3::new(x, 0.0, -400.0).expect("finite second point"),
        StackedFoldFixedSideV1::Left,
        StackedFoldRotationDirectionV1::Positive,
        90.0,
    )
    .expect("valid line candidate")
}

fn capture<'snapshot>(
    fixture: &Fixture,
    pose: &MaterialTreePose,
    snapshot: &'snapshot LayerOrderSnapshot,
    limits: StackedFoldReadLimitsV1,
) -> NativeStackedFoldReadGuardV1<'snapshot> {
    capture_stacked_fold_read_guard_v1(binding(fixture), input(fixture, pose, snapshot), limits)
        .expect("read guard")
}

#[test]
fn no_hinge_and_flat_tree_issue_complete_read_only_proposals() {
    for (with_hinge, line_x, expected_support) in [
        (false, 200.0, StackedFoldReadSupportV1::NoHingeSingleFace),
        (
            true,
            100.0,
            StackedFoldReadSupportV1::BitExactFlatEndpointTree,
        ),
    ] {
        let fixture = fixture(with_hinge);
        let pose = admitted_pose(&fixture);
        let guard = capture(
            &fixture,
            &pose,
            &fixture.layer_order,
            StackedFoldReadLimitsV1::default(),
        );
        let guard_clone = guard.clone();
        assert!(guard.same_guard(&guard_clone));
        assert_eq!(guard.model_id(), STACKED_FOLD_READ_GUARD_MODEL_ID_V1);
        assert_eq!(guard.support(), expected_support);
        assert!(!guard.authorizes_project_mutation());
        assert!(!guard.authorizes_apply_stacked_fold());

        let candidate = crossing_candidate(line_x);
        let proposal = propose_linear_stacked_fold_read_v1(
            &guard,
            binding(&fixture),
            input(&fixture, &pose, &fixture.layer_order),
            candidate,
            StackedFoldReadLimitsV1::default(),
        )
        .expect("read-only line proposal");
        let proposal_clone = proposal.clone();
        assert!(proposal.same_proposal(&proposal_clone));
        assert!(proposal.is_for_guard(&guard));
        assert_eq!(proposal.model_id(), STACKED_FOLD_READ_PROPOSAL_MODEL_ID_V1);
        assert_eq!(proposal.support(), expected_support);
        assert!(!proposal.crossed_cells().is_empty());
        assert!(!proposal.target_faces().is_empty());
        assert!(proposal.crossed_cells().iter().all(|cell| {
            let source = fixture
                .layer_order
                .overlap_cells
                .iter()
                .find(|source| source.cell_key.0 == cell.cell_key().canonical_bytes())
                .expect("proposal cell belongs to the exact layer certificate");
            cell.bottom_to_top_faces() == source.bottom_to_top_faces.as_slice()
                && !cell.bottom_to_top_faces().is_empty()
                && cell
                    .bottom_to_top_faces()
                    .iter()
                    .all(|face| proposal.target_faces().contains(face))
        }));
        assert!(!proposal.authorizes_project_mutation());
        assert!(!proposal.authorizes_apply_stacked_fold());
        let material_map = reverse_map_linear_stacked_fold_material_v1(
            &proposal,
            &guard,
            binding(&fixture),
            input(&fixture, &pose, &fixture.layer_order),
            StackedFoldReadLimitsV1::default(),
            StackedFoldMaterialMapLimitsV1::default(),
        )
        .expect("world line maps to every crossed material face");
        assert_eq!(
            material_map.model_id(),
            STACKED_FOLD_MATERIAL_MAP_MODEL_ID_V1
        );
        assert!(material_map.is_for_proposal(&proposal));
        assert_eq!(material_map.segments().len(), proposal.target_faces().len());
        assert!(
            material_map
                .segments()
                .iter()
                .zip(proposal.target_faces())
                .all(|(segment, face)| segment.face() == *face
                    && segment.start() != segment.end()
                    && matches!(segment.assignment(), EdgeKind::Mountain | EdgeKind::Valley)
                    && (segment.assignment() == EdgeKind::Mountain)
                        == (segment.fixed_side() == StackedFoldFixedSideV1::Left))
        );
        assert!(!material_map.authorizes_project_mutation());
        assert!(!material_map.authorizes_apply_stacked_fold());
        revalidate_linear_stacked_fold_read_proposal_v1(
            &proposal,
            &guard,
            binding(&fixture),
            input(&fixture, &pose, &fixture.layer_order),
            candidate,
            StackedFoldReadLimitsV1::default(),
        )
        .expect("immutable proposal revalidation");
    }
}

#[test]
fn material_reverse_mapping_is_bounded_and_revalidates_the_exact_proposal() {
    let fixture = fixture(false);
    let pose = admitted_pose(&fixture);
    let limits = StackedFoldReadLimitsV1::default();
    let guard = capture(&fixture, &pose, &fixture.layer_order, limits);
    let proposal = propose_linear_stacked_fold_read_v1(
        &guard,
        binding(&fixture),
        input(&fixture, &pose, &fixture.layer_order),
        crossing_candidate(200.0),
        limits,
    )
    .expect("proposal");

    for map_limits in [
        StackedFoldMaterialMapLimitsV1 {
            max_faces: 0,
            ..StackedFoldMaterialMapLimitsV1::default()
        },
        StackedFoldMaterialMapLimitsV1 {
            max_total_boundary_vertices: 0,
            ..StackedFoldMaterialMapLimitsV1::default()
        },
    ] {
        assert!(matches!(
            reverse_map_linear_stacked_fold_material_v1(
                &proposal,
                &guard,
                binding(&fixture),
                input(&fixture, &pose, &fixture.layer_order),
                limits,
                map_limits,
            ),
            Err(StackedFoldMaterialMapErrorV1::ResourceLimitExceeded)
        ));
    }

    let foreign_guard = capture(&fixture, &pose, &fixture.layer_order, limits);
    assert!(matches!(
        reverse_map_linear_stacked_fold_material_v1(
            &proposal,
            &foreign_guard,
            binding(&fixture),
            input(&fixture, &pose, &fixture.layer_order),
            limits,
            StackedFoldMaterialMapLimitsV1::default(),
        ),
        Err(StackedFoldMaterialMapErrorV1::ReadProposalInvalid)
    ));

    for (fixed_side, rotation_direction, expected_assignment) in [
        (
            StackedFoldFixedSideV1::Left,
            StackedFoldRotationDirectionV1::Positive,
            EdgeKind::Mountain,
        ),
        (
            StackedFoldFixedSideV1::Left,
            StackedFoldRotationDirectionV1::Negative,
            EdgeKind::Valley,
        ),
        (
            StackedFoldFixedSideV1::Right,
            StackedFoldRotationDirectionV1::Positive,
            EdgeKind::Valley,
        ),
        (
            StackedFoldFixedSideV1::Right,
            StackedFoldRotationDirectionV1::Negative,
            EdgeKind::Mountain,
        ),
    ] {
        let candidate = StackedFoldLinearCandidateV1::new(
            Point3::new(200.0, 0.0, 0.0).unwrap(),
            Point3::new(200.0, 0.0, -400.0).unwrap(),
            fixed_side,
            rotation_direction,
            90.0,
        )
        .unwrap();
        let proposal = propose_linear_stacked_fold_read_v1(
            &guard,
            binding(&fixture),
            input(&fixture, &pose, &fixture.layer_order),
            candidate,
            limits,
        )
        .unwrap();
        let material_map = reverse_map_linear_stacked_fold_material_v1(
            &proposal,
            &guard,
            binding(&fixture),
            input(&fixture, &pose, &fixture.layer_order),
            limits,
            StackedFoldMaterialMapLimitsV1::default(),
        )
        .unwrap();
        assert!(material_map.segments().iter().all(|segment| {
            segment.fixed_side() == fixed_side && segment.assignment() == expected_assignment
        }));
    }
}

#[test]
fn nonflat_endpoints_are_explicitly_unsupported() {
    let fixture = fixture(true);
    let root = fixture
        .layer_order
        .reference_face
        .expect("reference")
        .face_id;
    for angle in [90.0, 179.0, f64::from_bits(180.0_f64.to_bits() - 1)] {
        let pose = pose(&fixture, angle, Some(root));
        let error = capture_stacked_fold_read_guard_v1(
            binding(&fixture),
            input(&fixture, &pose, &fixture.layer_order),
            StackedFoldReadLimitsV1::default(),
        )
        .expect_err("only bit-exact 180 degrees is admitted");
        assert_eq!(error, StackedFoldReadErrorV1::UnsupportedNonFlatEndpoint);
        assert_eq!(
            error.failure_class(),
            StackedFoldReadFailureClassV1::Unsupported
        );
    }

    let mixed = three_panel_fixture();
    assert_eq!(mixed.model.hinges().len(), 2);
    let mixed_root = mixed
        .layer_order
        .reference_face
        .expect("three-panel reference")
        .face_id;
    let mixed_angles = CanonicalHingeAngles::new(
        mixed
            .model
            .hinges()
            .iter()
            .zip([180.0, 179.0])
            .map(|(hinge, angle)| HingeAngle::new(hinge.edge(), angle).expect("mixed angle"))
            .collect(),
    )
    .expect("canonical mixed angles");
    let mixed_pose = mixed
        .model
        .solve(Some(mixed_root), &mixed_angles)
        .expect("mixed three-panel pose");
    assert_eq!(
        capture_stacked_fold_read_guard_v1(
            binding(&mixed),
            input(&mixed, &mixed_pose, &mixed.layer_order),
            StackedFoldReadLimitsV1::default(),
        )
        .expect_err("one non-endpoint hinge blocks the entire tree"),
        StackedFoldReadErrorV1::UnsupportedNonFlatEndpoint
    );
}

#[test]
fn id_source_root_issuer_generation_and_aba_mismatches_fail_closed() {
    let fixture = fixture(true);
    let first_pose = admitted_pose(&fixture);
    let guard = capture(
        &fixture,
        &first_pose,
        &fixture.layer_order,
        StackedFoldReadLimitsV1::default(),
    );
    let candidate = crossing_candidate(100.0);
    let proposal = propose_linear_stacked_fold_read_v1(
        &guard,
        binding(&fixture),
        input(&fixture, &first_pose, &fixture.layer_order),
        candidate,
        StackedFoldReadLimitsV1::default(),
    )
    .expect("baseline proposal");

    for stale in [
        StackedFoldReadBindingV1::new(
            fixed_id(0x900),
            fixture.project,
            REVISION,
            POSE_GENERATION,
            LAYER_GENERATION,
        ),
        StackedFoldReadBindingV1::new(
            fixture.project_instance,
            fixture.project,
            REVISION + 1,
            POSE_GENERATION,
            LAYER_GENERATION,
        ),
        StackedFoldReadBindingV1::new(
            fixture.project_instance,
            fixture.project,
            REVISION,
            POSE_GENERATION + 1,
            LAYER_GENERATION,
        ),
        StackedFoldReadBindingV1::new(
            fixture.project_instance,
            fixture.project,
            REVISION,
            POSE_GENERATION,
            LAYER_GENERATION + 1,
        ),
    ] {
        assert_eq!(
            revalidate_stacked_fold_read_guard_v1(
                &guard,
                stale,
                input(&fixture, &first_pose, &fixture.layer_order),
                StackedFoldReadLimitsV1::default(),
            ),
            Err(StackedFoldReadErrorV1::AuthorityBindingMismatch)
        );
    }
    let wrong_project = StackedFoldReadBindingV1::new(
        fixture.project_instance,
        fixed_id(0x901),
        REVISION,
        POSE_GENERATION,
        LAYER_GENERATION,
    );
    assert_eq!(
        revalidate_stacked_fold_read_guard_v1(
            &guard,
            wrong_project,
            input(&fixture, &first_pose, &fixture.layer_order),
            StackedFoldReadLimitsV1::default(),
        ),
        Err(StackedFoldReadErrorV1::AuthorityBindingMismatch)
    );

    let fresh_same_guard = capture(
        &fixture,
        &first_pose,
        &fixture.layer_order,
        StackedFoldReadLimitsV1::default(),
    );
    assert!(!guard.same_guard(&fresh_same_guard));
    assert!(!proposal.is_for_guard(&fresh_same_guard));

    let second_pose = admitted_pose(&fixture);
    assert_eq!(
        revalidate_stacked_fold_read_guard_v1(
            &guard,
            binding(&fixture),
            input(&fixture, &second_pose, &fixture.layer_order),
            StackedFoldReadLimitsV1::default(),
        ),
        Err(StackedFoldReadErrorV1::AuthorityBindingMismatch)
    );
    let second_guard = capture(
        &fixture,
        &second_pose,
        &fixture.layer_order,
        StackedFoldReadLimitsV1::default(),
    );
    assert!(!guard.same_guard(&second_guard));
    assert!(!proposal.is_for_guard(&second_guard));
    assert_eq!(
        revalidate_linear_stacked_fold_read_proposal_v1(
            &proposal,
            &second_guard,
            binding(&fixture),
            input(&fixture, &second_pose, &fixture.layer_order),
            candidate,
            StackedFoldReadLimitsV1::default(),
        ),
        Err(StackedFoldReadErrorV1::AuthorityBindingMismatch)
    );

    let copied_snapshot = fixture.layer_order.clone();
    assert_eq!(copied_snapshot, fixture.layer_order);
    assert_eq!(
        revalidate_stacked_fold_read_guard_v1(
            &guard,
            binding(&fixture),
            input(&fixture, &first_pose, &copied_snapshot),
            StackedFoldReadLimitsV1::default(),
        ),
        Err(StackedFoldReadErrorV1::AuthorityBindingMismatch)
    );

    let foreign_model = MaterialTreeKinematicsModel::prepare(
        &fixture.pattern,
        &fixture.paper,
        &fixture.topology,
        TreeKinematicsLimits::default(),
    )
    .expect("equal but foreign issuer");
    let angles = CanonicalHingeAngles::new(
        foreign_model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 180.0).expect("angle"))
            .collect(),
    )
    .expect("canonical angles");
    let foreign_pose = foreign_model
        .solve(first_pose.fixed_face(), &angles)
        .expect("foreign pose");
    let foreign_input = FlatEndpointLayerOrderInputV1 {
        identity_namespace: fixture.project,
        source_revision: REVISION,
        paper: &fixture.paper,
        pattern: &fixture.pattern,
        model: &foreign_model,
        pose: &foreign_pose,
        layer_order: &fixture.layer_order,
    };
    assert_eq!(
        revalidate_stacked_fold_read_guard_v1(
            &guard,
            binding(&fixture),
            foreign_input,
            StackedFoldReadLimitsV1::default(),
        ),
        Err(StackedFoldReadErrorV1::AuthorityBindingMismatch)
    );

    let reference = first_pose.fixed_face().expect("tree root");
    let other_root = fixture
        .model
        .face_ids()
        .iter()
        .copied()
        .find(|face| *face != reference)
        .expect("second face");
    let wrong_root_pose = pose(&fixture, 180.0, Some(other_root));
    assert_eq!(
        capture_stacked_fold_read_guard_v1(
            binding(&fixture),
            input(&fixture, &wrong_root_pose, &fixture.layer_order),
            StackedFoldReadLimitsV1::default(),
        )
        .expect_err("a foreign root cannot be rebound"),
        StackedFoldReadErrorV1::LayerOrderIndeterminate(
            FlatEndpointLayerOrderAnchorErrorV1::ReferenceFaceMismatch
        )
    );
}

#[test]
fn certificate_changes_and_ambiguous_lines_never_become_proposals() {
    let fixture = fixture(true);
    let pose = admitted_pose(&fixture);

    let mut stale_source = fixture.layer_order.clone();
    stale_source.provenance.source.source_revision += 1;
    assert_eq!(
        capture_stacked_fold_read_guard_v1(
            binding(&fixture),
            input(&fixture, &pose, &stale_source),
            StackedFoldReadLimitsV1::default(),
        )
        .expect_err("stale source cannot be captured"),
        StackedFoldReadErrorV1::AuthorityBindingMismatch
    );

    let mut forged_cell = fixture.layer_order.clone();
    forged_cell.overlap_cells[0].cell_key.0[0] ^= 1;
    assert_eq!(
        capture_stacked_fold_read_guard_v1(
            binding(&fixture),
            input(&fixture, &pose, &forged_cell),
            StackedFoldReadLimitsV1::default(),
        )
        .expect_err("a changed certificate cell cannot be captured"),
        StackedFoldReadErrorV1::LayerOrderIndeterminate(
            FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch
        )
    );

    let guard = capture(
        &fixture,
        &pose,
        &fixture.layer_order,
        StackedFoldReadLimitsV1::default(),
    );
    assert_eq!(
        propose_linear_stacked_fold_read_v1(
            &guard,
            binding(&fixture),
            input(&fixture, &pose, &fixture.layer_order),
            crossing_candidate(0.0),
            StackedFoldReadLimitsV1::default(),
        )
        .expect_err("a boundary-coincident line remains indeterminate"),
        StackedFoldReadErrorV1::AmbiguousCellBoundary
    );
    let tangent = StackedFoldLinearCandidateV1::new(
        Point3::new(-100.0, 0.0, -100.0).expect("tangent first"),
        Point3::new(100.0, 0.0, 100.0).expect("tangent second"),
        StackedFoldFixedSideV1::Left,
        StackedFoldRotationDirectionV1::Positive,
        90.0,
    )
    .expect("valid tangent request");
    assert_eq!(
        propose_linear_stacked_fold_read_v1(
            &guard,
            binding(&fixture),
            input(&fixture, &pose, &fixture.layer_order),
            tangent,
            StackedFoldReadLimitsV1::default(),
        )
        .expect_err("a vertex-only tangent remains indeterminate"),
        StackedFoldReadErrorV1::AmbiguousCellBoundary
    );
    assert_eq!(
        propose_linear_stacked_fold_read_v1(
            &guard,
            binding(&fixture),
            input(&fixture, &pose, &fixture.layer_order),
            crossing_candidate(-10.0),
            StackedFoldReadLimitsV1::default(),
        )
        .expect_err("an outside line has no target cell"),
        StackedFoldReadErrorV1::NoCrossedLayerCell
    );

    let candidate = crossing_candidate(100.0);
    let proposal = propose_linear_stacked_fold_read_v1(
        &guard,
        binding(&fixture),
        input(&fixture, &pose, &fixture.layer_order),
        candidate,
        StackedFoldReadLimitsV1::default(),
    )
    .expect("baseline proposal");
    let changed_candidates = [
        StackedFoldLinearCandidateV1::new(
            Point3::new(f64::from_bits(100.0_f64.to_bits() + 1), 0.0, 0.0).expect("first"),
            candidate.second(),
            candidate.fixed_side(),
            candidate.rotation_direction(),
            candidate.requested_angle_degrees(),
        )
        .expect("first-point one-ULP candidate"),
        StackedFoldLinearCandidateV1::new(
            candidate.first(),
            Point3::new(
                candidate.second().x(),
                candidate.second().y(),
                f64::from_bits(candidate.second().z().to_bits() + 1),
            )
            .expect("second"),
            candidate.fixed_side(),
            candidate.rotation_direction(),
            candidate.requested_angle_degrees(),
        )
        .expect("second-point one-ULP candidate"),
        StackedFoldLinearCandidateV1::new(
            candidate.first(),
            candidate.second(),
            StackedFoldFixedSideV1::Right,
            candidate.rotation_direction(),
            candidate.requested_angle_degrees(),
        )
        .expect("changed fixed-side candidate"),
        StackedFoldLinearCandidateV1::new(
            candidate.first(),
            candidate.second(),
            candidate.fixed_side(),
            StackedFoldRotationDirectionV1::Negative,
            candidate.requested_angle_degrees(),
        )
        .expect("changed rotation candidate"),
        StackedFoldLinearCandidateV1::new(
            candidate.first(),
            candidate.second(),
            candidate.fixed_side(),
            candidate.rotation_direction(),
            f64::from_bits(candidate.requested_angle_degrees().to_bits() + 1),
        )
        .expect("angle one-ULP candidate"),
    ];
    for changed in changed_candidates {
        assert_eq!(
            revalidate_linear_stacked_fold_read_proposal_v1(
                &proposal,
                &guard,
                binding(&fixture),
                input(&fixture, &pose, &fixture.layer_order),
                changed,
                StackedFoldReadLimitsV1::default(),
            ),
            Err(StackedFoldReadErrorV1::AuthorityBindingMismatch)
        );
    }

    assert_eq!(
        StackedFoldLinearCandidateV1::new(
            Point3::new(0.0, 1.0, 0.0).expect("finite off-plane"),
            Point3::new(1.0, 1.0, 0.0).expect("finite off-plane"),
            StackedFoldFixedSideV1::Left,
            StackedFoldRotationDirectionV1::Positive,
            90.0,
        ),
        Err(StackedFoldReadErrorV1::InvalidLinearCandidate)
    );
}

#[test]
fn every_read_budget_admits_exact_equality_and_rejects_one_short() {
    let fixture = fixture(true);
    let pose = admitted_pose(&fixture);
    let candidate = crossing_candidate(100.0);
    let guard = capture(
        &fixture,
        &pose,
        &fixture.layer_order,
        StackedFoldReadLimitsV1::default(),
    );
    let baseline = propose_linear_stacked_fold_read_v1(
        &guard,
        binding(&fixture),
        input(&fixture, &pose, &fixture.layer_order),
        candidate,
        StackedFoldReadLimitsV1::default(),
    )
    .expect("baseline proposal");
    let work = baseline.work();
    let exact = StackedFoldReadLimitsV1 {
        max_scanned_cells: work.scanned_cells,
        max_total_boundary_vertices: work.total_boundary_vertices,
        max_total_layer_records: work.total_layer_records,
        max_orientation_tests: work.orientation_tests,
        max_exact_arithmetic_operations: work.exact_arithmetic_operations,
        max_exact_integer_bits: work.maximum_exact_integer_bits,
        max_total_exact_integer_bits: work.total_exact_integer_bits,
        max_retained_cells: work.retained_cells,
        max_retained_target_faces: work.retained_target_faces,
        ..StackedFoldReadLimitsV1::default()
    };
    propose_linear_stacked_fold_read_v1(
        &guard,
        binding(&fixture),
        input(&fixture, &pose, &fixture.layer_order),
        candidate,
        exact,
    )
    .expect("every proposal budget admits equality");

    for (limits, resource) in [
        (
            StackedFoldReadLimitsV1 {
                max_scanned_cells: work.scanned_cells - 1,
                ..exact
            },
            StackedFoldReadResourceV1::ScannedCells,
        ),
        (
            StackedFoldReadLimitsV1 {
                max_total_boundary_vertices: work.total_boundary_vertices - 1,
                ..exact
            },
            StackedFoldReadResourceV1::TotalBoundaryVertices,
        ),
        (
            StackedFoldReadLimitsV1 {
                max_total_layer_records: work.total_layer_records - 1,
                ..exact
            },
            StackedFoldReadResourceV1::TotalLayerRecords,
        ),
        (
            StackedFoldReadLimitsV1 {
                max_orientation_tests: work.orientation_tests - 1,
                ..exact
            },
            StackedFoldReadResourceV1::OrientationTests,
        ),
        (
            StackedFoldReadLimitsV1 {
                max_exact_arithmetic_operations: work.exact_arithmetic_operations - 1,
                ..exact
            },
            StackedFoldReadResourceV1::ExactArithmeticOperations,
        ),
        (
            StackedFoldReadLimitsV1 {
                max_exact_integer_bits: work.maximum_exact_integer_bits - 1,
                ..exact
            },
            StackedFoldReadResourceV1::ExactIntegerBits,
        ),
        (
            StackedFoldReadLimitsV1 {
                max_total_exact_integer_bits: work.total_exact_integer_bits - 1,
                ..exact
            },
            StackedFoldReadResourceV1::TotalExactIntegerBits,
        ),
        (
            StackedFoldReadLimitsV1 {
                max_retained_cells: work.retained_cells - 1,
                ..exact
            },
            StackedFoldReadResourceV1::RetainedCells,
        ),
        (
            StackedFoldReadLimitsV1 {
                max_retained_target_faces: work.retained_target_faces - 1,
                ..exact
            },
            StackedFoldReadResourceV1::RetainedTargetFaces,
        ),
    ] {
        assert!(matches!(
            propose_linear_stacked_fold_read_v1(
                &guard,
                binding(&fixture),
                input(&fixture, &pose, &fixture.layer_order),
                candidate,
                limits,
            ),
            Err(StackedFoldReadErrorV1::ResourceLimitExceeded {
                resource: actual,
                ..
            }) if actual == resource
        ));
    }

    let lower_fixture = off_center_fixture();
    let lower_pose = admitted_pose(&lower_fixture);
    let lower_guard = capture(
        &lower_fixture,
        &lower_pose,
        &lower_fixture.layer_order,
        StackedFoldReadLimitsV1::default(),
    );
    let anchor_work = lower_guard.layer_order_work();
    let source_vertices = lower_fixture.pattern.vertices.len();
    let source_edges = lower_fixture.pattern.edges.len();
    let paper_boundary_vertices = lower_fixture.paper.boundary_vertices.len();
    let boundary_vertices_per_cell = lower_fixture
        .layer_order
        .overlap_cells
        .iter()
        .map(|cell| cell.exact_boundary.len())
        .max()
        .expect("nonempty cell registry");
    let positive_lower_counts = [
        source_vertices,
        source_edges,
        paper_boundary_vertices,
        anchor_work.faces,
        anchor_work.hinges,
        anchor_work.cells,
        boundary_vertices_per_cell,
        anchor_work.total_boundary_vertices,
        anchor_work.total_layer_records,
        anchor_work.face_pair_orders,
        anchor_work.total_supporting_cells,
        anchor_work.exact_payload_bytes,
        anchor_work.maximum_exact_integer_bits,
        anchor_work.containment_orientation_tests,
        anchor_work.cell_separation_orientation_tests,
    ];
    assert!(
        positive_lower_counts.iter().all(|count| *count > 0),
        "the off-center fixture must exercise all 15 lower counters: {positive_lower_counts:?}"
    );
    let lower_exact = FlatEndpointLayerOrderLimitsV1 {
        max_source_vertices: source_vertices,
        max_source_edges: source_edges,
        max_paper_boundary_vertices: paper_boundary_vertices,
        max_faces: anchor_work.faces,
        max_hinges: anchor_work.hinges,
        max_cells: anchor_work.cells,
        max_boundary_vertices_per_cell: boundary_vertices_per_cell,
        max_total_boundary_vertices: anchor_work.total_boundary_vertices,
        max_total_layer_records: anchor_work.total_layer_records,
        max_face_pair_orders: anchor_work.face_pair_orders,
        max_total_supporting_cells: anchor_work.total_supporting_cells,
        max_exact_payload_bytes: anchor_work.exact_payload_bytes,
        max_exact_integer_bits: anchor_work.maximum_exact_integer_bits,
        max_containment_orientation_tests: anchor_work.containment_orientation_tests,
        max_cell_separation_orientation_tests: anchor_work.cell_separation_orientation_tests,
    };
    capture_stacked_fold_read_guard_v1(
        binding(&lower_fixture),
        input(&lower_fixture, &lower_pose, &lower_fixture.layer_order),
        StackedFoldReadLimitsV1 {
            flat_endpoint: lower_exact,
            ..Default::default()
        },
    )
    .expect("all 15 lower anchor limits admit exact equality");

    for (limits, resource) in [
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_source_vertices: source_vertices - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::SourceVertices,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_source_edges: source_edges - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::SourceEdges,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_paper_boundary_vertices: paper_boundary_vertices - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::PaperBoundaryVertices,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_faces: anchor_work.faces - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::Faces,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_hinges: anchor_work.hinges - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::Hinges,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_cells: anchor_work.cells - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::Cells,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_boundary_vertices_per_cell: boundary_vertices_per_cell - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::BoundaryVerticesPerCell,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_total_boundary_vertices: anchor_work.total_boundary_vertices - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::TotalBoundaryVertices,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_total_layer_records: anchor_work.total_layer_records - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::LayerRecords,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_face_pair_orders: anchor_work.face_pair_orders - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::FacePairOrders,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_total_supporting_cells: anchor_work.total_supporting_cells - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::SupportingCells,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_exact_payload_bytes: anchor_work.exact_payload_bytes - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::ExactPayloadBytes,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_exact_integer_bits: anchor_work.maximum_exact_integer_bits - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::ExactIntegerBits,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_containment_orientation_tests: anchor_work.containment_orientation_tests - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::ContainmentOrientationTests,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_cell_separation_orientation_tests: anchor_work
                    .cell_separation_orientation_tests
                    - 1,
                ..lower_exact
            },
            FlatEndpointLayerOrderResourceV1::CellSeparationOrientationTests,
        ),
    ] {
        assert!(matches!(
            capture_stacked_fold_read_guard_v1(
                binding(&lower_fixture),
                input(
                    &lower_fixture,
                    &lower_pose,
                    &lower_fixture.layer_order,
                ),
                StackedFoldReadLimitsV1 {
                    flat_endpoint: limits,
                    ..Default::default()
                },
            ),
            Err(StackedFoldReadErrorV1::LayerOrderIndeterminate(
                FlatEndpointLayerOrderAnchorErrorV1::ResourceLimitExceeded {
                    resource: actual,
                    ..
                }
            )) if actual == resource
        ));
    }
}
