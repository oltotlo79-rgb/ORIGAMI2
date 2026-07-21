use ori_collision::{
    FLAT_ENDPOINT_LAYER_ORDER_ANCHOR_MODEL_ID_V1, FlatEndpointLayerOrderAnchorErrorV1,
    FlatEndpointLayerOrderInputV1, FlatEndpointLayerOrderLimitsV1,
    FlatEndpointLayerOrderResourceV1, StaticCollisionLimits, StaticCollisionPairDisposition,
    anchor_flat_endpoint_layer_order_v1, diagnose_static_collision_geometry,
    diagnose_static_collision_geometry_with_flat_layer_order_v1,
    revalidate_flat_endpoint_layer_order_anchor_v1,
};
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_foldability::{
    ExactSign, GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, LayerOrderDerivation,
    LayerOrderSnapshot, analyze_global_flat_foldability,
};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, MaterialTreePose,
    TreeKinematicsLimits,
};
use ori_topology::{FaceExtractionInput, analyze_faces, analyze_local_flat_foldability};
use serde::de::DeserializeOwned;

const REVISION: u64 = 41;

#[test]
fn exact_layer_order_admits_the_shared_hinge_flat_stack() {
    let fixture = fixture(false);
    let pose = endpoint_pose(&fixture);
    let anchor = anchor_flat_endpoint_layer_order_v1(
        input(&fixture, &pose, &fixture.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("flat endpoint layer-order anchor");
    let diagnostic = diagnose_static_collision_geometry_with_flat_layer_order_v1(
        &fixture.model,
        &pose,
        0.0,
        StaticCollisionLimits::default(),
        &anchor,
    )
    .expect("layer-bound collision diagnostic");
    assert_eq!(diagnostic.expected_unordered_face_pairs(), 1);
    assert_eq!(diagnostic.allowed_pairs(), 1);
    assert_eq!(diagnostic.penetrating_pairs(), 0);
    assert_eq!(diagnostic.indeterminate_pairs(), 0);
    assert_eq!(
        diagnostic.pairs()[0].disposition(),
        StaticCollisionPairDisposition::Allowed
    );
}

#[test]
fn zabuton_five_faces_classify_eight_certified_stacks_and_two_separated_pairs() {
    let zabuton = zabuton_fixture();
    let pose = facewise_endpoint_pose(&zabuton);
    let unauthenticated = diagnose_static_collision_geometry(
        &zabuton.model,
        &pose,
        0.0,
        StaticCollisionLimits::default(),
    )
    .expect("zabuton diagnostic without layer authority");
    assert_eq!(unauthenticated.indeterminate_pairs(), 8);
    assert_eq!(unauthenticated.separated_pairs(), 2);

    let foreign = fixture(false);
    assert!(matches!(
        anchor_flat_endpoint_layer_order_v1(
            input(&zabuton, &pose, &foreign.layer_order),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::SourceIdentityMismatch)
    ));

    let mut tampered = zabuton.layer_order.clone();
    tampered
        .overlap_cells
        .iter_mut()
        .find(|cell| cell.bottom_to_top_faces.len() > 1)
        .expect("zabuton overlap cell")
        .bottom_to_top_faces
        .reverse();
    assert!(
        anchor_flat_endpoint_layer_order_v1(
            input(&zabuton, &pose, &tampered),
            FlatEndpointLayerOrderLimitsV1::default(),
        )
        .is_err()
    );

    let anchor = anchor_flat_endpoint_layer_order_v1(
        input(&zabuton, &pose, &zabuton.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("zabuton flat endpoint layer-order anchor");
    let diagnostic = diagnose_static_collision_geometry_with_flat_layer_order_v1(
        &zabuton.model,
        &pose,
        0.0,
        StaticCollisionLimits::default(),
        &anchor,
    )
    .expect("zabuton layer-bound collision diagnostic");

    assert_eq!(diagnostic.expected_unordered_face_pairs(), 10);
    assert_eq!(diagnostic.allowed_pairs(), 8);
    assert_eq!(diagnostic.separated_pairs(), 2);
    assert_eq!(diagnostic.indeterminate_pairs(), 0);
    assert_eq!(diagnostic.penetrating_pairs(), 0);
}

#[test]
fn three_face_two_hinge_chain_never_reports_flat_shared_hinges_as_penetrating() {
    let fixture = three_panel_fixture(false);
    let pose = facewise_endpoint_pose(&fixture);
    let unauthenticated = diagnose_static_collision_geometry(
        &fixture.model,
        &pose,
        0.0,
        StaticCollisionLimits::default(),
    )
    .expect("three-panel diagnostic without layer authority");
    let unauthenticated_hinges = unauthenticated
        .pairs()
        .iter()
        .filter(|pair| pair.topology().identifier() == "shared_hinge_edge")
        .collect::<Vec<_>>();
    assert_eq!(unauthenticated_hinges.len(), 2);
    assert!(unauthenticated_hinges.iter().all(|pair| {
        pair.disposition() == StaticCollisionPairDisposition::Indeterminate
            && pair.evidence().identifier() == "shared_feature_flat_stack"
    }));

    let anchor = anchor_flat_endpoint_layer_order_v1(
        input(&fixture, &pose, &fixture.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("three-panel flat endpoint layer-order anchor");
    let diagnostic = diagnose_static_collision_geometry_with_flat_layer_order_v1(
        &fixture.model,
        &pose,
        0.0,
        StaticCollisionLimits::default(),
        &anchor,
    )
    .expect("three-panel layer-bound collision diagnostic");
    let authenticated_hinges = diagnostic
        .pairs()
        .iter()
        .filter(|pair| pair.topology().identifier() == "shared_hinge_edge")
        .collect::<Vec<_>>();
    assert_eq!(authenticated_hinges.len(), 2);
    assert!(authenticated_hinges.iter().all(|pair| {
        pair.disposition() == StaticCollisionPairDisposition::Allowed
            && pair.evidence().identifier() == "shared_feature_flat_stack"
    }));
    // The two non-adjacent end panels genuinely overlap and retain their
    // separate no-shared-feature penetration result. That must never leak
    // onto either of the two shared hinges.
    assert_eq!(diagnostic.penetrating_pairs(), 1);
}

struct Fixture {
    project: ProjectId,
    paper: Paper,
    pattern: CreasePattern,
    layer_order: LayerOrderSnapshot,
    model: MaterialTreeKinematicsModel,
}

fn fixed_id<T: DeserializeOwned>(suffix: u64) -> T {
    serde_json::from_str(&format!("\"20000000-0000-4000-8000-{suffix:012x}\""))
        .expect("fixed UUID-backed ID")
}

fn fixture(reverse_source_collections: bool) -> Fixture {
    let project = fixed_id(1);
    let positions = [
        Point2::new(0.0, 0.0),
        Point2::new(200.0, 0.0),
        Point2::new(400.0, 0.0),
        Point2::new(400.0, 400.0),
        Point2::new(200.0, 400.0),
        Point2::new(0.0, 400.0),
    ];
    let mut vertices = positions
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
    edges.push(Edge {
        id: fixed_id(0x300),
        start: boundary[1],
        end: boundary[4],
        kind: EdgeKind::Mountain,
    });
    if reverse_source_collections {
        vertices.reverse();
        edges.reverse();
    }
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
    let topology = report.snapshot.expect("two-face topology");
    let local = analyze_local_flat_foldability(&paper, &pattern);
    let foldability = analyze_global_flat_foldability(
        GlobalFlatFoldabilityInput::current_with_geometry(
            project, &paper, &pattern, &topology, &local,
        ),
        GlobalFlatFoldabilityLimits::default(),
    )
    .expect("flat-foldability analysis executes");
    let layer_order = foldability
        .layer_order()
        .expect("single hinge has a layer order")
        .clone();
    let model = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("two-face material model");
    Fixture {
        project,
        paper,
        pattern,
        layer_order,
        model,
    }
}

fn three_panel_fixture(reverse_source_collections: bool) -> Fixture {
    let project = fixed_id(2);
    let positions = [
        Point2::new(0.0, 0.0),
        Point2::new(200.0, 0.0),
        Point2::new(400.0, 0.0),
        Point2::new(600.0, 0.0),
        Point2::new(600.0, 200.0),
        Point2::new(400.0, 200.0),
        Point2::new(200.0, 200.0),
        Point2::new(0.0, 200.0),
    ];
    let mut vertices = positions
        .into_iter()
        .enumerate()
        .map(|(index, position)| Vertex {
            id: fixed_id::<VertexId>(0x400 + index as u64),
            position,
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..vertices.len())
        .map(|index| Edge {
            id: fixed_id::<EdgeId>(0x500 + index as u64),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend([
        Edge {
            id: fixed_id(0x601),
            start: boundary[1],
            end: boundary[6],
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: fixed_id(0x602),
            start: boundary[2],
            end: boundary[5],
            kind: EdgeKind::Valley,
        },
    ]);
    if reverse_source_collections {
        vertices.reverse();
        edges.reverse();
    }
    let paper = Paper {
        boundary_vertices: boundary,
        thickness_mm: 0.1,
        ..Paper::default()
    };
    let pattern = CreasePattern { vertices, edges };
    let layer_order = derive_layer_order(project, &paper, &pattern);
    assert!(matches!(
        layer_order.provenance.derivation,
        LayerOrderDerivation::FacewiseCertificate { .. }
    ));
    assert_eq!(layer_order.material_faces.len(), 3);
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: project,
        source_revision: REVISION,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("three-face topology");
    assert_eq!(topology.faces.len(), 3);
    assert_eq!(topology.hinge_adjacency.len(), 2);
    let model = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("three-face material model");
    assert_eq!(model.face_ids().len(), 3);
    assert_eq!(model.hinges().len(), 2);
    Fixture {
        project,
        paper,
        pattern,
        layer_order,
        model,
    }
}

fn zabuton_fixture() -> Fixture {
    let project = fixed_id(3);
    let positions = [
        Point2::new(-100.0, -100.0),
        Point2::new(50.0, -200.0),
        Point2::new(100.0, -100.0),
        Point2::new(200.0, 50.0),
        Point2::new(100.0, 100.0),
        Point2::new(-50.0, 200.0),
        Point2::new(-100.0, 100.0),
        Point2::new(-200.0, -50.0),
    ];
    let vertices = positions
        .into_iter()
        .enumerate()
        .map(|(index, position)| Vertex {
            id: fixed_id::<VertexId>(0x700 + index as u64),
            position,
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id::<EdgeId>(0x800 + index as u64),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend(
        [(0, 2, 0x900), (2, 4, 0x901), (4, 6, 0x902), (6, 0, 0x903)].map(|(start, end, id)| Edge {
            id: fixed_id::<EdgeId>(id),
            start: boundary[start],
            end: boundary[end],
            kind: EdgeKind::Mountain,
        }),
    );
    let paper = Paper {
        boundary_vertices: boundary,
        thickness_mm: 0.1,
        ..Paper::default()
    };
    let pattern = CreasePattern { vertices, edges };
    let layer_order = derive_layer_order(project, &paper, &pattern);
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: project,
        source_revision: REVISION,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("zabuton topology");
    assert_eq!(topology.faces.len(), 5);
    assert_eq!(topology.hinge_adjacency.len(), 4);
    let model = MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("zabuton material tree");
    Fixture {
        project,
        paper,
        pattern,
        layer_order,
        model,
    }
}

fn derive_layer_order(
    project: ProjectId,
    paper: &Paper,
    pattern: &CreasePattern,
) -> LayerOrderSnapshot {
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: project,
        source_revision: REVISION,
        paper,
        pattern,
    });
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    let topology = report.snapshot.expect("face topology");
    let local = analyze_local_flat_foldability(paper, pattern);
    let foldability = analyze_global_flat_foldability(
        GlobalFlatFoldabilityInput::current_with_geometry(
            project, paper, pattern, &topology, &local,
        ),
        GlobalFlatFoldabilityLimits::default(),
    )
    .expect("flat-foldability analysis executes");
    foldability
        .layer_order()
        .expect("flat-foldable fixture has a layer order")
        .clone()
}

fn pose_at(fixture: &Fixture, root: ori_domain::FaceId, angle: f64) -> MaterialTreePose {
    let hinge = fixture.model.hinges()[0].edge();
    let angles = CanonicalHingeAngles::new(vec![
        HingeAngle::new(hinge, angle).expect("valid hinge angle"),
    ])
    .expect("canonical one-hinge vector");
    fixture
        .model
        .solve(Some(root), &angles)
        .expect("material pose")
}

fn uniform_pose_at(fixture: &Fixture, root: ori_domain::FaceId, angle: f64) -> MaterialTreePose {
    let angles = CanonicalHingeAngles::new(
        fixture
            .model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), angle).expect("valid hinge angle"))
            .collect(),
    )
    .expect("canonical complete hinge vector");
    fixture
        .model
        .solve(Some(root), &angles)
        .expect("material pose")
}

fn endpoint_pose(fixture: &Fixture) -> MaterialTreePose {
    pose_at(
        fixture,
        fixture
            .layer_order
            .reference_face
            .expect("reference face")
            .face_id,
        180.0,
    )
}

fn facewise_endpoint_pose(fixture: &Fixture) -> MaterialTreePose {
    uniform_pose_at(
        fixture,
        fixture
            .layer_order
            .reference_face
            .expect("reference face")
            .face_id,
        180.0,
    )
}

fn input<'source, 'snapshot>(
    fixture: &'source Fixture,
    pose: &'source MaterialTreePose,
    layer_order: &'snapshot LayerOrderSnapshot,
) -> FlatEndpointLayerOrderInputV1<'source, 'snapshot> {
    FlatEndpointLayerOrderInputV1 {
        identity_namespace: fixture.project,
        source_revision: REVISION,
        paper: &fixture.paper,
        pattern: &fixture.pattern,
        model: &fixture.model,
        pose,
        layer_order,
    }
}

#[test]
fn two_face_180_endpoint_anchors_complete_world_cells_and_revalidates() {
    let fixture = fixture(false);
    let pose = endpoint_pose(&fixture);
    let anchor = anchor_flat_endpoint_layer_order_v1(
        input(&fixture, &pose, &fixture.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("flat endpoint layer-order anchor");

    assert_eq!(
        anchor.model_id(),
        FLAT_ENDPOINT_LAYER_ORDER_ANCHOR_MODEL_ID_V1
    );
    assert_eq!(anchor.material_faces(), fixture.layer_order.material_faces);
    assert_eq!(
        anchor.cells().len(),
        fixture.layer_order.overlap_cells.len()
    );
    assert!(anchor.cells().iter().all(|cell| {
        cell.world_boundary().len() >= 3
            && cell.covering_faces().len() == cell.bottom_to_top_faces().len()
            && cell.world_boundary().iter().all(|point| {
                point.x().is_finite()
                    && point.y().to_bits() == 0.0_f64.to_bits()
                    && point.z().is_finite()
            })
    }));
    assert!(anchor.is_for_authorities(&fixture.model, &pose, &fixture.layer_order));
    revalidate_flat_endpoint_layer_order_anchor_v1(
        &anchor,
        input(&fixture, &pose, &fixture.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("immutable anchor revalidation");
}

#[test]
fn three_face_facewise_endpoint_anchors_complete_three_ply_order_and_revalidates() {
    let fixture = three_panel_fixture(false);
    let pose = facewise_endpoint_pose(&fixture);
    assert_eq!(pose.hinge_angles().len(), 2);
    assert!(
        pose.hinge_angles()
            .iter()
            .all(|angle| { angle.angle_degrees().to_bits() == 180.0_f64.to_bits() })
    );
    let anchor = anchor_flat_endpoint_layer_order_v1(
        input(&fixture, &pose, &fixture.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("three-face flat endpoint layer-order anchor");

    let LayerOrderDerivation::FacewiseCertificate {
        reference_face,
        overlap_cell_count,
        constraint_count,
    } = fixture.layer_order.provenance.derivation
    else {
        panic!("three faces must use the facewise certificate derivation");
    };
    assert_eq!(Some(reference_face), fixture.layer_order.reference_face);
    assert_eq!(overlap_cell_count, fixture.layer_order.overlap_cells.len());
    assert_eq!(
        Some(constraint_count),
        fixture
            .layer_order
            .proof_summary
            .map(|summary| summary.constraints)
    );

    let work = anchor.work();
    assert_eq!(work.faces, 3);
    assert_eq!(work.hinges, 2);
    assert_eq!(anchor.material_faces(), fixture.layer_order.material_faces);
    assert_eq!(
        anchor.cells().len(),
        fixture.layer_order.overlap_cells.len()
    );
    assert_eq!(
        fixture
            .layer_order
            .proof_summary
            .expect("facewise proof summary")
            .maximum_ply,
        3
    );

    let three_ply = anchor
        .cells()
        .iter()
        .find(|cell| cell.covering_faces().len() == 3)
        .expect("accordion has one three-ply overlap cell");
    let material_face_ids = fixture
        .layer_order
        .material_faces
        .iter()
        .map(|face| face.face_id)
        .collect::<Vec<_>>();
    assert_eq!(three_ply.covering_faces(), material_face_ids);
    assert_eq!(three_ply.bottom_to_top_faces().len(), 3);
    let mut ordered_set = three_ply.bottom_to_top_faces().to_vec();
    ordered_set.sort_unstable_by_key(ori_domain::FaceId::canonical_bytes);
    let mut material_set = material_face_ids;
    material_set.sort_unstable_by_key(ori_domain::FaceId::canonical_bytes);
    assert_eq!(ordered_set, material_set);

    let global_order = fixture
        .layer_order
        .global_bottom_to_top
        .as_ref()
        .expect("the one-cell accordion has a global layer order")
        .iter()
        .map(|face| face.face_id)
        .collect::<Vec<_>>();
    assert_eq!(three_ply.bottom_to_top_faces(), global_order);
    assert!(
        anchor
            .cells()
            .iter()
            .flat_map(|cell| cell.covering_faces())
            .all(|face| material_set.contains(face))
    );
    assert!(anchor.is_for_authorities(&fixture.model, &pose, &fixture.layer_order));
    revalidate_flat_endpoint_layer_order_anchor_v1(
        &anchor,
        input(&fixture, &pose, &fixture.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("three-face immutable anchor revalidation");
}

#[test]
fn three_face_facewise_anchor_is_invariant_to_source_storage_order() {
    let first = three_panel_fixture(false);
    let first_pose = facewise_endpoint_pose(&first);
    let first_anchor = anchor_flat_endpoint_layer_order_v1(
        input(&first, &first_pose, &first.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("baseline three-face anchor");

    let reordered = three_panel_fixture(true);
    let reordered_pose = facewise_endpoint_pose(&reordered);
    let reordered_anchor = anchor_flat_endpoint_layer_order_v1(
        input(&reordered, &reordered_pose, &reordered.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("source-reordered three-face anchor");

    assert!(matches!(
        first.layer_order.provenance.derivation,
        LayerOrderDerivation::FacewiseCertificate { .. }
    ));
    assert!(matches!(
        reordered.layer_order.provenance.derivation,
        LayerOrderDerivation::FacewiseCertificate { .. }
    ));
    assert_eq!(first.layer_order, reordered.layer_order);
    assert_eq!(
        first_anchor.source_fingerprint(),
        reordered_anchor.source_fingerprint()
    );
    assert_eq!(
        first_anchor.material_faces(),
        reordered_anchor.material_faces()
    );
    assert_eq!(first_anchor.cells(), reordered_anchor.cells());
    revalidate_flat_endpoint_layer_order_anchor_v1(
        &reordered_anchor,
        input(&reordered, &reordered_pose, &reordered.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("source-reordered three-face anchor revalidation");
}

#[test]
fn three_face_facewise_anchor_rejects_aba_and_certificate_tampering() {
    let fixture = three_panel_fixture(false);
    let first_pose = facewise_endpoint_pose(&fixture);
    let anchor = anchor_flat_endpoint_layer_order_v1(
        input(&fixture, &first_pose, &fixture.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("baseline three-face anchor");

    let second_pose = facewise_endpoint_pose(&fixture);
    assert!(!anchor.is_for_authorities(&fixture.model, &second_pose, &fixture.layer_order));
    assert_eq!(
        revalidate_flat_endpoint_layer_order_anchor_v1(
            &anchor,
            input(&fixture, &second_pose, &fixture.layer_order),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::AuthorityBindingMismatch)
    );

    let fresh_equal = derive_layer_order(fixture.project, &fixture.paper, &fixture.pattern);
    assert_eq!(fresh_equal, fixture.layer_order);
    assert!(!anchor.is_for_authorities(&fixture.model, &first_pose, &fresh_equal));
    assert_eq!(
        revalidate_flat_endpoint_layer_order_anchor_v1(
            &anchor,
            input(&fixture, &first_pose, &fresh_equal),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::AuthorityBindingMismatch)
    );

    let foreign = three_panel_fixture(false);
    let foreign_pose = facewise_endpoint_pose(&foreign);
    assert_eq!(
        revalidate_flat_endpoint_layer_order_anchor_v1(
            &anchor,
            input(&foreign, &foreign_pose, &fixture.layer_order),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::AuthorityBindingMismatch)
    );

    let mut forged_derivation = fixture.layer_order.clone();
    let LayerOrderDerivation::FacewiseCertificate {
        overlap_cell_count, ..
    } = &mut forged_derivation.provenance.derivation
    else {
        panic!("three faces must use the facewise certificate derivation");
    };
    *overlap_cell_count += 1;
    assert_eq!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &first_pose, &forged_derivation),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::LayerOrderModelMismatch)
    );

    let mut forged_summary = fixture.layer_order.clone();
    forged_summary
        .proof_summary
        .as_mut()
        .expect("facewise proof summary")
        .constraints += 1;
    assert_eq!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &first_pose, &forged_summary),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::LayerOrderModelMismatch)
    );

    let mut one_ulp_at_400mm_transform = fixture.layer_order.clone();
    let reference = fixture.layer_order.reference_face.expect("reference face");
    let transform = &mut one_ulp_at_400mm_transform
        .folded_faces
        .iter_mut()
        .find(|folded| folded.face == reference)
        .expect("reference folded face")
        .source_to_flat;
    transform.tx.sign = ExactSign::Positive;
    transform.tx.numerator_magnitude_be = vec![1];
    transform.tx.denominator_be = vec![0x10, 0, 0, 0, 0, 0];
    let one_ulp_result = anchor_flat_endpoint_layer_order_v1(
        input(&fixture, &first_pose, &one_ulp_at_400mm_transform),
        FlatEndpointLayerOrderLimitsV1::default(),
    );
    assert!(
        matches!(
            &one_ulp_result,
            Err(FlatEndpointLayerOrderAnchorErrorV1::FoldedFaceTransformMismatch { face })
                if *face == reference.face_id
        ),
        "{one_ulp_result:?}"
    );

    let mut forged_cell = fixture.layer_order.clone();
    forged_cell.overlap_cells[0].cell_key.0[0] ^= 1;
    assert_eq!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &first_pose, &forged_cell),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch)
    );

    let mut forged_order = fixture.layer_order.clone();
    forged_order
        .overlap_cells
        .iter_mut()
        .find(|cell| cell.bottom_to_top_faces.len() == 3)
        .expect("three-ply cell")
        .bottom_to_top_faces
        .reverse();
    assert!(matches!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &first_pose, &forged_order),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch
            | FlatEndpointLayerOrderAnchorErrorV1::GlobalOrderMismatch)
    ));
}

#[test]
fn exact_pose_and_snapshot_identity_reject_same_content_aba() {
    let fixture = fixture(false);
    let first_pose = endpoint_pose(&fixture);
    let anchor = anchor_flat_endpoint_layer_order_v1(
        input(&fixture, &first_pose, &fixture.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("first anchor");
    let cloned = anchor.clone();
    assert!(anchor.same_anchor(&cloned));

    let second_pose = endpoint_pose(&fixture);
    assert!(!anchor.is_for_authorities(&fixture.model, &second_pose, &fixture.layer_order));
    assert_eq!(
        revalidate_flat_endpoint_layer_order_anchor_v1(
            &anchor,
            input(&fixture, &second_pose, &fixture.layer_order),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::AuthorityBindingMismatch)
    );

    let copied_snapshot = fixture.layer_order.clone();
    assert_eq!(copied_snapshot, fixture.layer_order);
    assert!(!anchor.is_for_authorities(&fixture.model, &first_pose, &copied_snapshot));
    assert_eq!(
        revalidate_flat_endpoint_layer_order_anchor_v1(
            &anchor,
            input(&fixture, &first_pose, &copied_snapshot),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::AuthorityBindingMismatch)
    );

    let foreign = self::fixture(false);
    let foreign_pose = endpoint_pose(&foreign);
    assert_eq!(
        revalidate_flat_endpoint_layer_order_anchor_v1(
            &anchor,
            input(&foreign, &foreign_pose, &foreign.layer_order),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::AuthorityBindingMismatch)
    );
}

#[test]
fn stale_foreign_registry_and_wrong_root_fail_closed() {
    let fixture = fixture(false);
    let pose = endpoint_pose(&fixture);

    let mut stale = fixture.layer_order.clone();
    stale.provenance.source.source_revision += 1;
    assert_eq!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &pose, &stale),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::SourceIdentityMismatch)
    );

    let mut foreign_paper = fixture.paper.clone();
    foreign_paper.cutting_allowed = !foreign_paper.cutting_allowed;
    let mut foreign_input = input(&fixture, &pose, &fixture.layer_order);
    foreign_input.paper = &foreign_paper;
    assert_eq!(
        anchor_flat_endpoint_layer_order_v1(
            foreign_input,
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::SourceFingerprintMismatch)
    );

    let mut foreign_registry = fixture.layer_order.clone();
    foreign_registry.material_faces.swap(0, 1);
    assert_eq!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &pose, &foreign_registry),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::MaterialFaceRegistryMismatch)
    );

    let reference = fixture
        .layer_order
        .reference_face
        .expect("reference")
        .face_id;
    let other_root = fixture
        .model
        .face_ids()
        .iter()
        .copied()
        .find(|face| *face != reference)
        .expect("other face");
    let wrong_root = pose_at(&fixture, other_root, 180.0);
    assert_eq!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &wrong_root, &fixture.layer_order),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::ReferenceFaceMismatch)
    );
}

#[test]
fn nonendpoint_malformed_exact_cell_and_order_forgery_fail_closed() {
    let fixture = fixture(false);
    let not_flat = pose_at(
        &fixture,
        fixture
            .layer_order
            .reference_face
            .expect("reference")
            .face_id,
        179.0,
    );
    assert!(matches!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &not_flat, &fixture.layer_order),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::NotBitExactFlatEndpoint { .. })
    ));

    let pose = endpoint_pose(&fixture);
    let mut malformed = fixture.layer_order.clone();
    malformed.folded_faces[0]
        .source_to_flat
        .m00
        .denominator_be
        .clear();
    assert_eq!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &pose, &malformed),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::InvalidExactPayload)
    );

    let mut nonfinite_world = fixture.layer_order.clone();
    nonfinite_world.folded_faces[0].source_to_flat.tx.sign = ExactSign::Positive;
    nonfinite_world.folded_faces[0]
        .source_to_flat
        .tx
        .numerator_magnitude_be = [1_u8]
        .into_iter()
        .chain(std::iter::repeat_n(0_u8, 128))
        .collect();
    assert_eq!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &pose, &nonfinite_world),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::InvalidExactPayload)
    );

    let mut missing_cell = fixture.layer_order.clone();
    missing_cell.overlap_cells.pop().expect("at least one cell");
    assert!(matches!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &pose, &missing_cell),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(
            FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch
                | FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch
                | FlatEndpointLayerOrderAnchorErrorV1::LayerOrderModelMismatch
        )
    ));

    let mut forged_key = fixture.layer_order.clone();
    forged_key.overlap_cells[0].cell_key.0[0] ^= 1;
    assert_eq!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &pose, &forged_key),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::CellCompletenessMismatch)
    );

    let mut forged_order = fixture.layer_order.clone();
    let cell = forged_order
        .overlap_cells
        .iter_mut()
        .find(|cell| cell.bottom_to_top_faces.len() == 2)
        .expect("two-ply cell");
    cell.bottom_to_top_faces.reverse();
    assert!(matches!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &pose, &forged_order),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch
            | FlatEndpointLayerOrderAnchorErrorV1::GlobalOrderMismatch)
    ));

    let mut forged_two_cycle = fixture.layer_order.clone();
    let mut reverse = forged_two_cycle.face_pair_orders[0].clone();
    std::mem::swap(&mut reverse.lower_face, &mut reverse.upper_face);
    forged_two_cycle.face_pair_orders.push(reverse);
    assert_eq!(
        anchor_flat_endpoint_layer_order_v1(
            input(&fixture, &pose, &forged_two_cycle),
            FlatEndpointLayerOrderLimitsV1::default(),
        ),
        Err(FlatEndpointLayerOrderAnchorErrorV1::CellOrderMismatch)
    );
}

#[test]
fn resource_limits_are_charged_and_source_storage_order_is_invariant() {
    let first = fixture(false);
    let first_pose = endpoint_pose(&first);
    let baseline = anchor_flat_endpoint_layer_order_v1(
        input(&first, &first_pose, &first.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("baseline anchor");
    let work = baseline.work();
    assert!(work.exact_payload_bytes > 0);
    assert!(work.containment_orientation_tests > 0);
    let exact_limits = FlatEndpointLayerOrderLimitsV1 {
        max_source_vertices: first.pattern.vertices.len(),
        max_source_edges: first.pattern.edges.len(),
        max_paper_boundary_vertices: first.paper.boundary_vertices.len(),
        max_faces: work.faces,
        max_hinges: work.hinges,
        max_cells: work.cells,
        max_boundary_vertices_per_cell: first
            .layer_order
            .overlap_cells
            .iter()
            .map(|cell| cell.exact_boundary.len())
            .max()
            .expect("nonempty cell registry"),
        max_total_boundary_vertices: work.total_boundary_vertices,
        max_total_layer_records: work.total_layer_records,
        max_face_pair_orders: work.face_pair_orders,
        max_total_supporting_cells: work.total_supporting_cells,
        max_exact_payload_bytes: work.exact_payload_bytes,
        max_exact_integer_bits: work.maximum_exact_integer_bits,
        max_containment_orientation_tests: work.containment_orientation_tests,
        max_cell_separation_orientation_tests: work.cell_separation_orientation_tests,
    };
    anchor_flat_endpoint_layer_order_v1(
        input(&first, &first_pose, &first.layer_order),
        exact_limits,
    )
    .expect("all resource limits admit exact equality");

    for (limits, resource) in [
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_faces: work.faces - 1,
                ..Default::default()
            },
            FlatEndpointLayerOrderResourceV1::Faces,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_cells: work.cells - 1,
                ..Default::default()
            },
            FlatEndpointLayerOrderResourceV1::Cells,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_exact_payload_bytes: work.exact_payload_bytes - 1,
                ..Default::default()
            },
            FlatEndpointLayerOrderResourceV1::ExactPayloadBytes,
        ),
        (
            FlatEndpointLayerOrderLimitsV1 {
                max_containment_orientation_tests: work.containment_orientation_tests - 1,
                ..Default::default()
            },
            FlatEndpointLayerOrderResourceV1::ContainmentOrientationTests,
        ),
    ] {
        assert!(matches!(
            anchor_flat_endpoint_layer_order_v1(
                input(&first, &first_pose, &first.layer_order),
                limits,
            ),
            Err(FlatEndpointLayerOrderAnchorErrorV1::ResourceLimitExceeded {
                resource: actual,
                ..
            }) if actual == resource
        ));
    }

    let reversed = fixture(true);
    let reversed_pose = endpoint_pose(&reversed);
    let reordered = anchor_flat_endpoint_layer_order_v1(
        input(&reversed, &reversed_pose, &reversed.layer_order),
        FlatEndpointLayerOrderLimitsV1::default(),
    )
    .expect("reordered source anchor");
    assert_eq!(
        baseline.source_fingerprint(),
        reordered.source_fingerprint()
    );
    assert_eq!(baseline.material_faces(), reordered.material_faces());
    assert_eq!(baseline.cells(), reordered.cells());
}
