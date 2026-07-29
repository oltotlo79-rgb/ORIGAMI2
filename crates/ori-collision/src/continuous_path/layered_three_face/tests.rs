use std::cell::Cell;

use super::{
    LayeredThreeFaceContinuousErrorV1, LayeredThreeFaceContinuousLimitsV1,
    LayeredThreeFaceTestCheckpointHookV1, LayeredThreeFaceTestCheckpointPhaseV1,
    certify_layered_three_face_continuous_path_with_control_v1,
    clear_layered_three_face_test_checkpoint_hook_v1, matches_three_face_schedule_v1,
    set_layered_three_face_test_checkpoint_hook_v1, strict_axis_gap_v1,
    strictly_separated_registry_pair_v1,
};
use crate::{
    CooperativeOperationControlV1, NonFlatFacePairOrderStructuralV1,
    NonFlatFoldedFaceStructuralRefV1, NonFlatLayerOrderStructuralSourceV1,
    NonFlatOverlapCellStructuralRefV1, StackedFoldInitialLayerOrderSourceV1,
    StackedFoldPathDiagnosticErrorV1, StaticCollisionLimits,
    diagnose_static_collision_geometry_with_control_v1,
    prepare_stacked_fold_initial_sample_layer_admission_with_control_v1,
};
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_foldability::{ExactAffineTransform, ExactPointValue, ExactRationalValue, ExactSign};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MaterialTreeKinematicsModel, MaterialTreePose,
    OutwardIntervalV1, TreeKinematicsLimits,
};
use ori_topology::{FaceExtractionInput, analyze_faces};

mod resource_and_control;

struct LayeredThreeFaceTestCheckpointHookCleanupV1;

impl Drop for LayeredThreeFaceTestCheckpointHookCleanupV1 {
    fn drop(&mut self) {
        clear_layered_three_face_test_checkpoint_hook_v1();
    }
}

fn arm_layered_three_face_test_checkpoint_hook_v1(
    hook: LayeredThreeFaceTestCheckpointHookV1,
) -> LayeredThreeFaceTestCheckpointHookCleanupV1 {
    set_layered_three_face_test_checkpoint_hook_v1(hook);
    LayeredThreeFaceTestCheckpointHookCleanupV1
}

type LeafPoint = (VertexId, [OutwardIntervalV1; 3]);

struct ReadOnceInitialLayerSourceV1 {
    faces: Vec<FaceId>,
    transforms: Vec<ExactAffineTransform>,
    cells: Vec<(Vec<Point2>, Vec<ExactPointValue>, FaceId, FaceId)>,
    orders: Vec<NonFlatFacePairOrderStructuralV1>,
    fixed_face: FaceId,
    hinge_angles: Vec<(EdgeId, u64)>,
    observed: Cell<u64>,
}

impl ReadOnceInitialLayerSourceV1 {
    fn observe(&self, bit: u64) {
        assert_eq!(
            self.observed.get() & bit,
            0,
            "source observation was read twice"
        );
        self.observed.set(self.observed.get() | bit);
    }
}

impl NonFlatLayerOrderStructuralSourceV1 for ReadOnceInitialLayerSourceV1 {
    fn material_face_count(&self) -> usize {
        self.observe(1 << 0);
        self.faces.len()
    }

    fn material_face_id(&self, index: usize) -> Option<FaceId> {
        self.observe(1 << (1 + index));
        self.faces.get(index).copied()
    }

    fn folded_face_count(&self) -> usize {
        self.observe(1 << 4);
        self.faces.len()
    }

    fn folded_face(&self, index: usize) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>> {
        self.observe(1 << (5 + index));
        self.faces
            .get(index)
            .zip(self.transforms.get(index))
            .map(|(face, transform)| NonFlatFoldedFaceStructuralRefV1 {
                face_id: *face,
                dropped_world_axis: 2,
                source_to_plane: transform,
            })
    }

    fn overlap_cell_count(&self) -> usize {
        self.observe(1 << 8);
        self.cells.len()
    }

    fn overlap_cell(&self, index: usize) -> Option<NonFlatOverlapCellStructuralRefV1<'_>> {
        self.observe(1 << (9 + index));
        self.cells
            .get(index)
            .map(
                |(boundary, exact_boundary, lower, upper)| NonFlatOverlapCellStructuralRefV1 {
                    boundary,
                    exact_boundary,
                    lower_face: *lower,
                    upper_face: *upper,
                },
            )
    }

    fn face_pair_order_count(&self) -> usize {
        self.observe(1 << 12);
        self.orders.len()
    }

    fn face_pair_order(&self, index: usize) -> Option<NonFlatFacePairOrderStructuralV1> {
        self.observe(1 << (13 + index));
        self.orders.get(index).copied()
    }
}

impl StackedFoldInitialLayerOrderSourceV1 for ReadOnceInitialLayerSourceV1 {
    fn tested_face_pairs_v1(&self) -> usize {
        self.observe(1 << 16);
        self.faces.len() * self.faces.len().saturating_sub(1) / 2
    }

    fn fixed_face_v1(&self) -> Option<FaceId> {
        self.observe(1 << 17);
        Some(self.fixed_face)
    }

    fn hinge_angle_count_v1(&self) -> usize {
        self.observe(1 << 18);
        self.hinge_angles.len()
    }

    fn hinge_angle_v1(&self, index: usize) -> Option<(EdgeId, u64)> {
        self.observe(1 << (19 + index));
        self.hinge_angles.get(index).copied()
    }

    fn paper_thickness_bits_v1(&self) -> u64 {
        self.observe(1 << 21);
        0.0_f64.to_bits()
    }
}

fn point(x: (f64, f64)) -> LeafPoint {
    (
        VertexId::new(),
        [
            OutwardIntervalV1::new(x.0, x.1).unwrap(),
            OutwardIntervalV1::new(0.0, 0.0).unwrap(),
            OutwardIntervalV1::new(0.0, 0.0).unwrap(),
        ],
    )
}

fn fixed_id<T: serde::de::DeserializeOwned>(prefix: &str, index: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-4000-{prefix}-{index:012x}\"")).unwrap()
}

fn three_face_two_hinge_model_v1() -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (50.0, 0.0),
        (250.0, 0.0),
        (300.0, 0.0),
        (300.0, 100.0),
        (250.0, 100.0),
        (50.0, 100.0),
        (0.0, 100.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("b320", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("b321", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend([
        Edge {
            id: fixed_id("b321", 9),
            start: boundary[1],
            end: boundary[6],
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: fixed_id("b321", 10),
            start: boundary[2],
            end: boundary[5],
            kind: EdgeKind::Valley,
        },
    ]);
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id::<ProjectId>("b322", 1),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.unwrap(),
        TreeKinematicsLimits::default(),
    )
    .unwrap()
}

fn schedule_v1(
    model: &MaterialTreeKinematicsModel,
    moving_source: f64,
    moving_target: f64,
) -> (CanonicalHingeAngles, CanonicalHingeAngles) {
    let edges = model.hinges();
    let source = CanonicalHingeAngles::new(vec![
        HingeAngle::new(edges[0].edge(), moving_source).unwrap(),
        HingeAngle::new(edges[1].edge(), 180.0).unwrap(),
    ])
    .unwrap();
    let target = CanonicalHingeAngles::new(vec![
        HingeAngle::new(edges[0].edge(), moving_target).unwrap(),
        HingeAngle::new(edges[1].edge(), 180.0).unwrap(),
    ])
    .unwrap();
    (source, target)
}

fn exact_integer_v1(value: u8) -> ExactRationalValue {
    ExactRationalValue {
        sign: if value == 0 {
            ExactSign::Zero
        } else {
            ExactSign::Positive
        },
        numerator_magnitude_be: (value != 0).then_some(vec![value]).unwrap_or_default(),
        denominator_be: vec![1],
    }
}

fn exact_identity_v1() -> ExactAffineTransform {
    ExactAffineTransform {
        m00: exact_integer_v1(1),
        m01: exact_integer_v1(0),
        m10: exact_integer_v1(0),
        m11: exact_integer_v1(1),
        tx: exact_integer_v1(0),
        ty: exact_integer_v1(0),
    }
}

fn exact_triangle_v1() -> (Vec<Point2>, Vec<ExactPointValue>) {
    let boundary = vec![
        Point2::new(0.0, 0.0),
        Point2::new(1.0, 0.0),
        Point2::new(0.0, 1.0),
    ];
    let exact_boundary = vec![
        ExactPointValue {
            x: exact_integer_v1(0),
            y: exact_integer_v1(0),
        },
        ExactPointValue {
            x: exact_integer_v1(1),
            y: exact_integer_v1(0),
        },
        ExactPointValue {
            x: exact_integer_v1(0),
            y: exact_integer_v1(1),
        },
    ];
    (boundary, exact_boundary)
}

fn admission_source_v1(
    model: &MaterialTreeKinematicsModel,
    pose: &ori_kinematics::MaterialTreePose,
) -> ReadOnceInitialLayerSourceV1 {
    let snapshot = diagnose_static_collision_geometry_with_control_v1(
        model,
        pose,
        0.0,
        StaticCollisionLimits::default(),
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("three-face source pose static diagnostic");
    assert!(
        snapshot.pairs().iter().all(|pair| {
            pair.disposition() != crate::StaticCollisionPairDisposition::Indeterminate
                || pair.evidence() == crate::IntersectionEvidenceV2::SharedFeatureFlatStack
        }),
        "fixture has an initial indeterminate pair that initial-layer admission cannot order: {:?}",
        snapshot.pairs(),
    );
    assert!(
        snapshot.penetrating_pairs() == 0
            && snapshot.indeterminate_pairs() == 1
            && snapshot.candidate_excluded_pairs() == 0,
        "fixture static admission preconditions are not met: {:?}",
        snapshot.pairs(),
    );
    assert!(
        snapshot.pairs().iter().any(|pair| {
            pair.topology() == crate::TopologyRelation::SharedHingeEdge
                && pair.evidence() == crate::IntersectionEvidenceV2::SharedFeatureContact
                && pair.disposition() == crate::StaticCollisionPairDisposition::Allowed
        }) && snapshot.pairs().iter().any(|pair| {
            pair.topology() == crate::TopologyRelation::NoSharedFeature
                && pair.disposition() != crate::StaticCollisionPairDisposition::Indeterminate
        }),
        "fixture must leave the stationary hinge and nonadjacent pair outside layer admission: {:?}",
        snapshot.pairs(),
    );
    let mut orders = snapshot
        .pairs()
        .iter()
        .filter(|pair| pair.disposition() == crate::StaticCollisionPairDisposition::Indeterminate)
        .map(|pair| {
            let (lower_face, upper_face) =
                if pair.first_face().canonical_bytes() < pair.second_face().canonical_bytes() {
                    (pair.first_face(), pair.second_face())
                } else {
                    (pair.second_face(), pair.first_face())
                };
            NonFlatFacePairOrderStructuralV1 {
                lower_face,
                upper_face,
            }
        })
        .collect::<Vec<_>>();
    orders.sort_unstable_by_key(|order| {
        (
            order.lower_face.canonical_bytes(),
            order.upper_face.canonical_bytes(),
        )
    });
    assert!(!orders.is_empty(), "fixture needs initial flat-stack pairs");
    let cells = orders
        .iter()
        .map(|order| {
            let (boundary, exact_boundary) = exact_triangle_v1();
            (boundary, exact_boundary, order.lower_face, order.upper_face)
        })
        .collect();
    ReadOnceInitialLayerSourceV1 {
        faces: model.face_ids().to_vec(),
        transforms: model
            .face_ids()
            .iter()
            .map(|_| exact_identity_v1())
            .collect(),
        cells,
        orders,
        fixed_face: pose.fixed_face().expect("fixture fixed face"),
        hinge_angles: pose
            .hinge_angles()
            .iter()
            .map(|angle| (angle.edge(), angle.angle_degrees().to_bits()))
            .collect(),
        observed: Cell::new(0),
    }
}

struct LayeredThreeFaceFixtureV1 {
    model: MaterialTreeKinematicsModel,
    initial_angles: CanonicalHingeAngles,
    source_pose: MaterialTreePose,
    target_angles: CanonicalHingeAngles,
    admission: crate::NativeStackedFoldInitialSampleLayerAdmissionV1<ReadOnceInitialLayerSourceV1>,
    limits: LayeredThreeFaceContinuousLimitsV1,
}

fn layered_three_face_fixture_v1() -> LayeredThreeFaceFixtureV1 {
    let model = three_face_two_hinge_model_v1();
    let (initial_angles, target_angles) = schedule_v1(&model, 0.0, 45.0);
    let source_pose = model
        .solve(Some(model.face_ids()[0]), &initial_angles)
        .expect("fixture source pose");
    let source = admission_source_v1(&model, &source_pose);
    let admission = prepare_stacked_fold_initial_sample_layer_admission_with_control_v1(
        &model,
        &source_pose,
        0.0,
        StaticCollisionLimits::default(),
        &source,
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("controlled initial-layer admission");
    LayeredThreeFaceFixtureV1 {
        model,
        initial_angles,
        source_pose,
        target_angles,
        admission,
        limits: LayeredThreeFaceContinuousLimitsV1::default(),
    }
}

#[test]
fn strict_outward_gap_accepts_only_a_positive_gap() {
    assert!(strict_axis_gap_v1(
        &[point((0.0, 1.0))],
        &[point((2.0, 3.0))],
        0
    ));
    assert!(!strict_axis_gap_v1(
        &[point((0.0, 1.0))],
        &[point((1.0, 2.0))],
        0
    ));
    assert!(!strict_axis_gap_v1(
        &[point((0.0, 2.0))],
        &[point((1.0, 3.0))],
        0
    ));
}

#[test]
fn real_three_face_two_hinge_schedule_has_one_complete_nonduplicated_partition() {
    let model = three_face_two_hinge_model_v1();
    let (source, target) = schedule_v1(&model, 0.0, 45.0);
    let pose = model.solve(Some(model.face_ids()[0]), &source).unwrap();
    let partition = matches_three_face_schedule_v1(&model, &pose, &target).unwrap();
    let mut pairs = [
        partition.stationary_pair,
        partition.moving_pair,
        partition.nonadjacent_pair,
    ];
    pairs.sort_unstable_by_key(|pair| (pair[0].canonical_bytes(), pair[1].canonical_bytes()));
    assert!(pairs.windows(2).all(|entries| entries[0] != entries[1]));
    assert_eq!(pairs.len(), 3);
}

#[test]
fn three_face_certificate_binds_its_model_pose_and_admission_issuer() {
    let LayeredThreeFaceFixtureV1 {
        model,
        initial_angles,
        source_pose,
        target_angles,
        admission,
        limits,
    } = layered_three_face_fixture_v1();
    let certificate = certify_layered_three_face_continuous_path_with_control_v1(
        &model,
        &source_pose,
        &target_angles,
        &admission,
        limits,
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("controlled dyadic three-face certificate");
    assert_eq!(
        certificate.is_for_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Ok(true)
    );

    let alternate_source = admission_source_v1(&model, &source_pose);
    let alternate_admission = prepare_stacked_fold_initial_sample_layer_admission_with_control_v1(
        &model,
        &source_pose,
        0.0,
        StaticCollisionLimits::default(),
        &alternate_source,
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("separate source admission");
    assert_eq!(
        certificate.is_for_with_control_v1(
            &model,
            &source_pose,
            &target_angles,
            &alternate_admission,
            limits,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Ok(false),
        "a same-shaped source cannot impersonate the retained admission issuer"
    );

    let other_model = three_face_two_hinge_model_v1();
    let other_source_pose = other_model
        .solve(Some(other_model.face_ids()[0]), &initial_angles)
        .expect("separate model source pose");
    assert_eq!(
        certificate.is_for_with_control_v1(
            &other_model,
            &other_source_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Ok(false),
        "a same-shaped model cannot reuse the original admission"
    );

    let target_pose = model
        .solve(source_pose.fixed_face(), &target_angles)
        .expect("fixture target pose");
    assert_eq!(
        certificate.is_for_with_control_v1(
            &model,
            &target_pose,
            &target_angles,
            &admission,
            limits,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Ok(false),
        "initial-layer evidence is restricted to t = 0"
    );
}

#[test]
fn real_three_face_two_hinge_rejects_nonzero_source_and_flat_stationary_hinges() {
    let model = three_face_two_hinge_model_v1();
    let (nonzero_source, target) = schedule_v1(&model, 1.0, 45.0);
    let nonzero_pose = model
        .solve(Some(model.face_ids()[0]), &nonzero_source)
        .unwrap();
    assert!(matches_three_face_schedule_v1(&model, &nonzero_pose, &target).is_none());

    let edges = model.hinges();
    let source = CanonicalHingeAngles::new(vec![
        HingeAngle::new(edges[0].edge(), 0.0).unwrap(),
        HingeAngle::new(edges[1].edge(), 0.0).unwrap(),
    ])
    .unwrap();
    let target = CanonicalHingeAngles::new(vec![
        HingeAngle::new(edges[0].edge(), 45.0).unwrap(),
        HingeAngle::new(edges[1].edge(), 0.0).unwrap(),
    ])
    .unwrap();
    let pose = model.solve(Some(model.face_ids()[0]), &source).unwrap();
    assert!(matches_three_face_schedule_v1(&model, &pose, &target).is_none());
}
