use ori_core::{
    ExpectedStackedFoldCreaseV1, FaceLineageLimits, PreparedStackedFoldInitialPoseV1,
    StackedFoldGeometryLimitsV1, StackedFoldTopologyBuildLimitsV1, build_stacked_fold_topology_v1,
    create_rectangular_sheet, prepare_stacked_fold_geometry_candidate_v1,
    prepare_stacked_fold_initial_pose_v1, prepare_stacked_fold_target_model_v1,
};
use ori_domain::{CreasePattern, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, VertexId};
use ori_foldability::{
    GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, LayerOrderSnapshot,
    analyze_global_flat_foldability, fold_model_fingerprint_v1,
};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, TreeKinematicsLimits,
};
use ori_topology::{
    FaceExtractionInput, TopologySnapshot, analyze_faces, analyze_local_flat_foldability,
};
use std::collections::{HashMap, HashSet};

pub(crate) const SOURCE_REVISION: u64 = 100;
pub(crate) const BASELINE_TARGET_REVISION: u64 = SOURCE_REVISION + 1;
pub(crate) const CHANGED_TARGET_REVISION: u64 = SOURCE_REVISION + 2;
pub(crate) const PAPER_THICKNESS_MM: f64 = 1.0;
pub(crate) const REQUESTED_ANGLE_DEGREES: f64 = 0.5;
pub(crate) const FACE_COUNT: usize = 8;

const SOURCE_CREASE_ENDPOINTS: (usize, usize) = (0, 2);

pub(crate) struct ProductionTargetV1 {
    initial: PreparedStackedFoldInitialPoseV1,
    pub(crate) topology: TopologySnapshot,
    pub(crate) revision: u64,
    pub(crate) fingerprint: [u8; 32],
}

impl ProductionTargetV1 {
    pub(crate) fn model(&self) -> &MaterialTreeKinematicsModel {
        self.initial.target().model()
    }

    pub(crate) fn pose(&self) -> &ori_kinematics::MaterialTreePose {
        self.initial.pose()
    }

    pub(crate) fn candidate_pattern(&self) -> &CreasePattern {
        &self.initial.target().geometry().candidate().pattern
    }

    pub(crate) fn moving_hinges(&self) -> Vec<EdgeId> {
        self.model()
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect()
    }
}

pub(crate) struct ProductionFixtureV1 {
    pub(crate) baseline: ProductionTargetV1,
    pub(crate) changed: ProductionTargetV1,
    pub(crate) changed_vertex: VertexId,
    pub(crate) changed_edge: EdgeId,
}

fn point(index: usize) -> Point2 {
    Point2::new(index as f64, (index * index) as f64)
}

fn rectangular_point(index: usize) -> Point2 {
    let vertex_count = FACE_COUNT + 2;
    let first_branch = vertex_count / 3;
    let second_branch = vertex_count * 2 / 3;
    if index == 0 {
        Point2::new(0.0, 0.0)
    } else if index == 1 {
        Point2::new(400.0, 0.0)
    } else if index <= first_branch {
        Point2::new(400.0, 400.0 * (index - 1) as f64 / first_branch as f64)
    } else if index == first_branch + 1 {
        Point2::new(400.0, 400.0)
    } else if index <= second_branch {
        let segments = second_branch - first_branch;
        let offset = index - first_branch - 1;
        Point2::new(400.0 * (segments - offset) as f64 / segments as f64, 400.0)
    } else if index == second_branch + 1 {
        Point2::new(0.0, 400.0)
    } else if index < vertex_count {
        let segments = vertex_count - second_branch - 1;
        let offset = index - second_branch - 1;
        Point2::new(0.0, 400.0 * (segments - offset) as f64 / segments as f64)
    } else {
        panic!("virtual production boundary index")
    }
}

fn diagonal_indices() -> Vec<(usize, usize)> {
    let vertex_count = FACE_COUNT + 2;
    let first_branch = vertex_count / 3;
    let second_branch = vertex_count * 2 / 3;
    let mut diagonals = vec![
        (0, first_branch),
        (first_branch, second_branch),
        (second_branch, 0),
    ];
    diagonals.extend((2..first_branch).map(|end| (0, end)));
    diagonals.extend((first_branch + 2..second_branch).map(|end| (first_branch, end)));
    diagonals.extend((second_branch + 2..vertex_count).map(|end| (second_branch, end)));
    diagonals.sort_unstable();
    diagonals.dedup();
    assert_eq!(diagonals.len(), FACE_COUNT - 1);
    diagonals
}

fn expected_crease(
    endpoints: (usize, usize),
    offset: usize,
    point_at: fn(usize) -> Point2,
) -> ExpectedStackedFoldCreaseV1 {
    ExpectedStackedFoldCreaseV1 {
        start: point_at(endpoints.0),
        end: point_at(endpoints.1),
        kind: if offset.is_multiple_of(2) {
            EdgeKind::Mountain
        } else {
            EdgeKind::Valley
        },
    }
}

fn production_source(
    identity: ProjectId,
) -> (
    CreasePattern,
    Paper,
    VertexId,
    EdgeId,
    Vec<ExpectedStackedFoldCreaseV1>,
) {
    let (boundary_pattern, mut paper) = create_rectangular_sheet(400.0, 400.0, false)
        .expect("production rectangular sheet")
        .into_parts();
    paper.thickness_mm = PAPER_THICKNESS_MM;
    let diagonals = diagonal_indices();
    let retained_index = diagonals
        .iter()
        .position(|endpoints| *endpoints == SOURCE_CREASE_ENDPOINTS)
        .expect("source leaf diagonal");
    let source_creases = diagonals
        .iter()
        .enumerate()
        .map(|(index, endpoints)| expected_crease(*endpoints, index, rectangular_point))
        .collect::<Vec<_>>();
    let mut source = build_stacked_fold_topology_v1(
        identity,
        SOURCE_REVISION - 1,
        &boundary_pattern,
        &paper,
        &source_creases,
        StackedFoldTopologyBuildLimitsV1::default(),
    )
    .expect("production source crease graph");
    let mut generated_vertices = Vec::with_capacity(FACE_COUNT + 2);
    for index in 0..FACE_COUNT + 2 {
        let vertex = source
            .pattern
            .vertices
            .iter_mut()
            .find(|vertex| {
                vertex.position.x.to_bits() == rectangular_point(index).x.to_bits()
                    && vertex.position.y.to_bits() == rectangular_point(index).y.to_bits()
            })
            .expect("production-generated boundary vertex");
        generated_vertices.push(vertex.id);
        vertex.position = point(index);
    }
    let changed_vertex = generated_vertices[SOURCE_CREASE_ENDPOINTS.1];
    let root_vertex = generated_vertices[SOURCE_CREASE_ENDPOINTS.0];
    let changed_edge = source
        .pattern
        .edges
        .iter()
        .find(|edge| {
            edge.kind != EdgeKind::Boundary
                && [edge.start, edge.end]
                    .into_iter()
                    .all(|vertex| vertex == root_vertex || vertex == changed_vertex)
        })
        .expect("production source leaf crease")
        .id;
    source
        .pattern
        .edges
        .retain(|edge| edge.kind == EdgeKind::Boundary || edge.id == changed_edge);
    let expected = diagonals
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != retained_index)
        .map(|(index, endpoints)| expected_crease(*endpoints, index, point))
        .collect();
    (
        source.pattern,
        source.paper,
        changed_vertex,
        changed_edge,
        expected,
    )
}

fn proven_layer_order(
    identity: ProjectId,
    revision: u64,
    pattern: &CreasePattern,
    paper: &Paper,
) -> LayerOrderSnapshot {
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: identity,
        source_revision: revision,
        paper,
        pattern,
    })
    .snapshot
    .expect("source topology");
    let local = analyze_local_flat_foldability(paper, pattern);
    analyze_global_flat_foldability(
        GlobalFlatFoldabilityInput::current_with_geometry(
            identity, paper, pattern, &topology, &local,
        ),
        GlobalFlatFoldabilityLimits::default(),
    )
    .expect("global source analysis")
    .layer_order()
    .expect("possible source layer order")
    .clone()
}

fn prepare_target(
    identity: ProjectId,
    source_revision: u64,
    source_pattern: &CreasePattern,
    source_paper: &Paper,
    expected: &[ExpectedStackedFoldCreaseV1],
) -> ProductionTargetV1 {
    let source_layer_order =
        proven_layer_order(identity, source_revision, source_pattern, source_paper);
    let independently_built = build_stacked_fold_topology_v1(
        identity,
        source_revision,
        source_pattern,
        source_paper,
        expected,
        StackedFoldTopologyBuildLimitsV1::default(),
    )
    .expect("production target topology builder");
    let geometry = prepare_stacked_fold_geometry_candidate_v1(
        identity,
        source_revision,
        source_pattern,
        source_paper,
        &source_layer_order,
        expected,
        StackedFoldTopologyBuildLimitsV1::default(),
        FaceLineageLimits::default(),
        StackedFoldGeometryLimitsV1::default(),
    )
    .expect("production stacked-fold geometry");
    assert_eq!(
        geometry.candidate(),
        &independently_built,
        "the lineage/geometry proof must bind the independently regenerated topology"
    );
    let candidate = geometry.candidate();
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: identity,
        source_revision: source_revision + 1,
        paper: &candidate.paper,
        pattern: &candidate.pattern,
    })
    .snapshot
    .expect("production target topology");
    let source_topology = analyze_faces(FaceExtractionInput {
        identity_namespace: identity,
        source_revision,
        paper: source_paper,
        pattern: source_pattern,
    })
    .snapshot
    .expect("source topology");
    let source_model = MaterialTreeKinematicsModel::prepare(
        source_pattern,
        source_paper,
        &source_topology,
        TreeKinematicsLimits::default(),
    )
    .expect("source model");
    let source_angles = CanonicalHingeAngles::new(
        source_model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).expect("zero source angle"))
            .collect(),
    )
    .expect("canonical source angles");
    let source_pose = source_model
        .solve(Some(source_model.face_ids()[0]), &source_angles)
        .expect("source pose");
    let target = prepare_stacked_fold_target_model_v1(geometry, TreeKinematicsLimits::default())
        .expect("production target model");
    let initial = prepare_stacked_fold_initial_pose_v1(target, &source_model, &source_pose)
        .expect("production target initial pose");
    let candidate = initial.target().geometry().candidate();
    let fingerprint = fold_model_fingerprint_v1(&candidate.pattern, &candidate.paper).0;
    ProductionTargetV1 {
        initial,
        topology,
        revision: source_revision + 1,
        fingerprint,
    }
}

pub(crate) fn production_fixture(identity: ProjectId) -> ProductionFixtureV1 {
    let (source_pattern, source_paper, changed_vertex, changed_edge, expected) =
        production_source(identity);
    let baseline = prepare_target(
        identity,
        SOURCE_REVISION,
        &source_pattern,
        &source_paper,
        &expected,
    );
    let mut changed_pattern = source_pattern.clone();
    changed_pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == changed_vertex)
        .expect("edited production vertex")
        .position
        .y += 0.25;
    let changed = prepare_target(
        identity,
        SOURCE_REVISION + 1,
        &changed_pattern,
        &source_paper,
        &expected,
    );
    ProductionFixtureV1 {
        baseline,
        changed,
        changed_vertex,
        changed_edge,
    }
}

pub(crate) fn footprint_map(
    topology: &TopologySnapshot,
) -> HashMap<FaceId, (HashSet<VertexId>, HashSet<EdgeId>)> {
    topology
        .faces
        .iter()
        .map(|face| {
            (
                face.id,
                (
                    face.outer
                        .half_edges
                        .iter()
                        .flat_map(|half_edge| [half_edge.origin, half_edge.destination])
                        .collect(),
                    face.outer
                        .half_edges
                        .iter()
                        .map(|half_edge| half_edge.edge)
                        .collect(),
                ),
            )
        })
        .collect()
}
