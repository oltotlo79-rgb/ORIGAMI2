use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use super::*;
use crate::{
    NonFlatFacePairOrderStructuralV1, NonFlatFoldedFaceStructuralRefV1,
    NonFlatLayerOrderStructuralSourceV1, NonFlatOverlapCellStructuralRefV1,
    prepare_stacked_fold_initial_sample_layer_admission_with_control_v1,
};
use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId};
use ori_foldability::{ExactAffineTransform, ExactPointValue, ExactRationalValue, ExactSign};
use ori_kinematics::{
    HingeAngle, MaterialTreeDyadicIntervalLimitsV1, OutwardIntervalV1, TreeKinematicsLimits,
};
use ori_topology::{FaceExtractionInput, analyze_faces};

use super::super::layered_chain_common::{
    LayeredChainNonadjacentIntervalPairV1, pair_key_v1,
    verify_layered_chain_nonadjacent_gaps_with_control_v1,
};

struct FiveFaceInitialLayerSourceV1 {
    faces: Vec<FaceId>,
    transforms: Vec<ExactAffineTransform>,
    cells: Vec<(Vec<Point2>, Vec<ExactPointValue>, FaceId, FaceId)>,
    orders: Vec<NonFlatFacePairOrderStructuralV1>,
    fixed_face: FaceId,
    hinge_angles: Vec<(EdgeId, u64)>,
}

impl NonFlatLayerOrderStructuralSourceV1 for FiveFaceInitialLayerSourceV1 {
    fn material_face_count(&self) -> usize {
        self.faces.len()
    }

    fn material_face_id(&self, index: usize) -> Option<FaceId> {
        self.faces.get(index).copied()
    }

    fn folded_face_count(&self) -> usize {
        self.faces.len()
    }

    fn folded_face(&self, index: usize) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>> {
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
        self.cells.len()
    }

    fn overlap_cell(&self, index: usize) -> Option<NonFlatOverlapCellStructuralRefV1<'_>> {
        self.cells
            .get(index)
            .map(|(boundary, exact_boundary, lower_face, upper_face)| {
                NonFlatOverlapCellStructuralRefV1 {
                    boundary,
                    exact_boundary,
                    lower_face: *lower_face,
                    upper_face: *upper_face,
                }
            })
    }

    fn face_pair_order_count(&self) -> usize {
        self.orders.len()
    }

    fn face_pair_order(&self, index: usize) -> Option<NonFlatFacePairOrderStructuralV1> {
        self.orders.get(index).copied()
    }
}

impl StackedFoldInitialLayerOrderSourceV1 for FiveFaceInitialLayerSourceV1 {
    fn tested_face_pairs_v1(&self) -> usize {
        FIVE_ALL_PAIR_COUNT_V1
    }

    fn fixed_face_v1(&self) -> Option<FaceId> {
        Some(self.fixed_face)
    }

    fn hinge_angle_count_v1(&self) -> usize {
        self.hinge_angles.len()
    }

    fn hinge_angle_v1(&self, index: usize) -> Option<(EdgeId, u64)> {
        self.hinge_angles.get(index).copied()
    }

    fn paper_thickness_bits_v1(&self) -> u64 {
        0.0_f64.to_bits()
    }
}

fn fixed_id<T: serde::de::DeserializeOwned>(prefix: &str, index: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-4000-{prefix}-{index:012x}\"")).unwrap()
}

fn five_face_four_hinge_model_v1(namespace: u64) -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (500.0, 0.0),
        (1_000.0, 0.0),
        (1_900.0, 0.0),
        (2_400.0, 0.0),
        (3_300.0, 0.0),
        (3_300.0, 300.0),
        (2_800.0, 300.0),
        (1_500.0, 300.0),
        (1_400.0, 300.0),
        (100.0, 300.0),
        (0.0, 300.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("b520", namespace * 100 + index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("b521", namespace * 100 + index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend([
        Edge {
            id: fixed_id("b521", namespace * 100 + 21),
            start: boundary[1],
            end: boundary[10],
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: fixed_id("b521", namespace * 100 + 22),
            start: boundary[2],
            end: boundary[9],
            kind: EdgeKind::Valley,
        },
        Edge {
            id: fixed_id("b521", namespace * 100 + 23),
            start: boundary[3],
            end: boundary[8],
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: fixed_id("b521", namespace * 100 + 24),
            start: boundary[4],
            end: boundary[7],
            kind: EdgeKind::Valley,
        },
    ]);
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id::<ProjectId>("b522", namespace),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.issues.is_empty(), "{:#?}", report.issues);
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("five-face topology"),
        TreeKinematicsLimits::default(),
    )
    .expect("five-face material tree")
}

fn schedule_for_moving_hinge_v1(
    model: &MaterialTreeKinematicsModel,
    moving_hinge: EdgeId,
    moving_target: f64,
) -> (CanonicalHingeAngles, CanonicalHingeAngles) {
    let build = |moving_angle| {
        CanonicalHingeAngles::new(
            model
                .hinges()
                .iter()
                .map(|hinge| {
                    HingeAngle::new(
                        hinge.edge(),
                        if hinge.edge() == moving_hinge {
                            moving_angle
                        } else {
                            180.0
                        },
                    )
                    .expect("bounded angle")
                })
                .collect(),
        )
        .expect("canonical angles")
    };
    (build(0.0), build(moving_target))
}

fn minimum_nonadjacent_gap_depth_v1(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target_angles: &CanonicalHingeAngles,
    pairs: [[FaceId; 2]; FIVE_NONADJACENT_PAIR_COUNT_V1],
) -> Option<u8> {
    let interval_limits = LayeredFiveFaceChainContinuousLimitsV1::default().interval_limits;
    let maximum_depth =
        u8::try_from(super::super::MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1.ilog2()).ok()?;
    for depth in 0..=maximum_depth {
        let leaf_count = 1_usize << depth;
        if (0..leaf_count).all(|index| {
            model
                .prepare_dyadic_face_vertex_intervals_v1(
                    source_pose,
                    target_angles,
                    depth,
                    index as u64,
                    interval_limits,
                )
                .ok()
                .is_some_and(|registry| {
                    verify_layered_chain_nonadjacent_registry_gaps_with_control_v1(
                        &registry,
                        &pairs,
                        FIVE_NONADJACENT_PAIR_COUNT_V1,
                        FIVE_NONADJACENT_PAIR_COUNT_V1,
                        &CooperativeOperationControlV1::unbounded(),
                    )
                    .is_ok()
                })
        }) {
            return Some(depth);
        }
    }
    None
}

fn production_source_v1(
    model: &MaterialTreeKinematicsModel,
) -> (EdgeId, CanonicalHingeAngles, MaterialTreePose, u8) {
    let direct_hinges: [(EdgeId, [FaceId; 2]); FIVE_HINGE_COUNT_V1] =
        std::array::from_fn(|index| {
            let hinge = &model.hinges()[index];
            (
                hinge.edge(),
                canonical_pair_v1(hinge.left_face(), hinge.right_face()),
            )
        });
    let partition = validate_linear_chain_hinges_v1(
        model.face_ids(),
        &direct_hinges,
        FIVE_FACE_COUNT_V1,
        FIVE_ALL_PAIR_COUNT_V1,
    )
    .expect("bounded chain")
    .expect("linear five-face chain");
    let nonadjacent_pairs: [[FaceId; 2]; FIVE_NONADJACENT_PAIR_COUNT_V1] =
        partition.nonadjacent_pairs.try_into().expect("six pairs");
    let mut observed = Vec::new();
    for hinge in model.hinges() {
        let moving_hinge = hinge.edge();
        let (source_angles, target_angles) =
            schedule_for_moving_hinge_v1(model, moving_hinge, 90.0);
        let moving_pair = canonical_pair_v1(hinge.left_face(), hinge.right_face());
        let stationary_pairs = model
            .hinges()
            .iter()
            .filter(|candidate| candidate.edge() != moving_hinge)
            .map(|candidate| canonical_pair_v1(candidate.left_face(), candidate.right_face()))
            .collect::<Vec<_>>();
        for fixed_face in model.face_ids() {
            let Ok(source_pose) = model.solve(Some(*fixed_face), &source_angles) else {
                continue;
            };
            let Ok(snapshot) = diagnose_static_collision_geometry_with_control_v1(
                model,
                &source_pose,
                0.0,
                StaticCollisionLimits::default(),
                &CooperativeOperationControlV1::unbounded(),
            ) else {
                continue;
            };
            let Ok(target_pose) = model.solve(Some(*fixed_face), &target_angles) else {
                continue;
            };
            let moving_boundary = direct_hinge_boundary_only_open_interval_theorem_v1(
                model,
                &source_pose,
                &target_pose,
                moving_hinge,
                moving_pair,
                &CooperativeOperationControlV1::unbounded(),
            )
            .unwrap_or(false);
            let flat_pairs = snapshot
                .pairs()
                .iter()
                .filter(|pair| {
                    pair.disposition() == crate::StaticCollisionPairDisposition::Indeterminate
                        && pair.evidence() == crate::IntersectionEvidenceV2::SharedFeatureFlatStack
                })
                .collect::<Vec<_>>();
            let stationary_flat = flat_pairs.len() == FIVE_STATIONARY_HINGE_COUNT_V1
                && flat_pairs.iter().all(|pair| {
                    stationary_pairs
                        .contains(&canonical_pair_v1(pair.first_face(), pair.second_face()))
                });
            let moving_boundary_at_source = snapshot.pairs().iter().any(|pair| {
                canonical_pair_v1(pair.first_face(), pair.second_face()) == moving_pair
                    && pair.disposition() == crate::StaticCollisionPairDisposition::Allowed
                    && pair.evidence() == crate::IntersectionEvidenceV2::SharedFeatureContact
            });
            let gap_depth = moving_boundary
                .then(|| {
                    minimum_nonadjacent_gap_depth_v1(
                        model,
                        &source_pose,
                        &target_angles,
                        nonadjacent_pairs,
                    )
                })
                .flatten();
            if snapshot.penetrating_pairs() == 0
                && snapshot.candidate_excluded_pairs() == 0
                && snapshot.indeterminate_pairs() == FIVE_STATIONARY_HINGE_COUNT_V1
                && snapshot.touching_pairs() == 0
                && snapshot.allowed_pairs() == 1
                && snapshot.separated_pairs() == FIVE_NONADJACENT_PAIR_COUNT_V1
                && stationary_flat
                && moving_boundary_at_source
                && moving_boundary
                && let Some(gap_depth) = gap_depth
            {
                return (moving_hinge, source_angles, source_pose, gap_depth);
            }
            observed.push((
                moving_hinge,
                *fixed_face,
                moving_boundary,
                gap_depth,
                snapshot.pairs().to_vec(),
            ));
        }
    }
    panic!("no production-valid five-face source candidate: {observed:?}");
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
    pose: &MaterialTreePose,
) -> FiveFaceInitialLayerSourceV1 {
    let snapshot = diagnose_static_collision_geometry_with_control_v1(
        model,
        pose,
        0.0,
        StaticCollisionLimits::default(),
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("five-face source diagnosis");
    let mut orders = snapshot
        .pairs()
        .iter()
        .filter(|pair| pair.disposition() == crate::StaticCollisionPairDisposition::Indeterminate)
        .map(|pair| {
            let [lower_face, upper_face] = canonical_pair_v1(pair.first_face(), pair.second_face());
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
    assert_eq!(orders.len(), FIVE_STATIONARY_HINGE_COUNT_V1);
    let cells = orders
        .iter()
        .map(|order| {
            let (boundary, exact_boundary) = exact_triangle_v1();
            (boundary, exact_boundary, order.lower_face, order.upper_face)
        })
        .collect();
    FiveFaceInitialLayerSourceV1 {
        faces: model.face_ids().to_vec(),
        transforms: model
            .face_ids()
            .iter()
            .map(|_| exact_identity_v1())
            .collect(),
        cells,
        orders,
        fixed_face: pose.fixed_face().expect("fixed face"),
        hinge_angles: pose
            .hinge_angles()
            .iter()
            .map(|angle| (angle.edge(), angle.angle_degrees().to_bits()))
            .collect(),
    }
}

#[derive(Clone)]
struct FiveFaceFixtureV1 {
    model: MaterialTreeKinematicsModel,
    moving_hinge: EdgeId,
    source_angles: CanonicalHingeAngles,
    source_pose: MaterialTreePose,
    target_angles: CanonicalHingeAngles,
    admission: NativeStackedFoldInitialSampleLayerAdmissionV1<FiveFaceInitialLayerSourceV1>,
    limits: LayeredFiveFaceChainContinuousLimitsV1,
}

fn fixture_v1() -> FiveFaceFixtureV1 {
    static FIXTURE: std::sync::OnceLock<FiveFaceFixtureV1> = std::sync::OnceLock::new();
    FIXTURE.get_or_init(build_fixture_v1).clone()
}

fn build_fixture_v1() -> FiveFaceFixtureV1 {
    let model = five_face_four_hinge_model_v1(1);
    let (moving_hinge, source_angles, source_pose, dyadic_depth) = production_source_v1(&model);
    let (_, target_angles) = schedule_for_moving_hinge_v1(&model, moving_hinge, 90.0);
    let source = admission_source_v1(&model, &source_pose);
    let admission = prepare_stacked_fold_initial_sample_layer_admission_with_control_v1(
        &model,
        &source_pose,
        0.0,
        StaticCollisionLimits::default(),
        &source,
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("five-face initial layer admission");
    FiveFaceFixtureV1 {
        model,
        moving_hinge,
        source_angles,
        source_pose,
        target_angles,
        admission,
        limits: LayeredFiveFaceChainContinuousLimitsV1 {
            dyadic_depth,
            max_leaves: 1_usize << dyadic_depth,
            ..LayeredFiveFaceChainContinuousLimitsV1::default()
        },
    }
}

fn canonical_chain_v1() -> (
    [FaceId; FIVE_FACE_COUNT_V1],
    [(EdgeId, [FaceId; 2]); FIVE_HINGE_COUNT_V1],
) {
    let faces = std::array::from_fn(|_| FaceId::new());
    let hinges = std::array::from_fn(|index| {
        (
            EdgeId::new(),
            canonical_pair_v1(faces[index], faces[index + 1]),
        )
    });
    (faces, hinges)
}

#[test]
fn five_face_linear_chain_partitions_all_ten_pairs_once() {
    let (faces, hinges) = canonical_chain_v1();
    let partition = validate_linear_chain_hinges_v1(
        &faces,
        &hinges,
        FIVE_FACE_COUNT_V1,
        FIVE_ALL_PAIR_COUNT_V1,
    )
    .expect("bounded validation")
    .expect("five-face linear chain");
    assert_eq!(partition.direct_pairs.len(), FIVE_HINGE_COUNT_V1);
    assert_eq!(
        partition.nonadjacent_pairs.len(),
        FIVE_NONADJACENT_PAIR_COUNT_V1
    );
    let mut all = partition.direct_pairs;
    all.extend(partition.nonadjacent_pairs);
    all.sort_unstable_by_key(|pair| (pair[0].canonical_bytes(), pair[1].canonical_bytes()));
    assert_eq!(all.len(), FIVE_ALL_PAIR_COUNT_V1);
    assert!(all.windows(2).all(|pairs| pairs[0] != pairs[1]));
}

#[test]
fn five_face_schedule_requires_one_moving_and_three_stationary_hinges() {
    let (_, hinges) = canonical_chain_v1();
    let source = hinges.map(|(edge, _)| (edge, 180.0));
    let mut target = source;
    let mut moving_source = source;
    moving_source[1].1 = 0.0;
    target[1].1 = 45.0;
    let schedule = validate_single_moving_flat_chain_schedule_v1(
        &hinges,
        &moving_source,
        &target,
        FIVE_HINGE_COUNT_V1,
        FIVE_HINGE_COUNT_V1,
    )
    .expect("bounded schedule")
    .expect("one moving schedule");
    assert_eq!(schedule.iter().filter(|hinge| hinge.moving).count(), 1);

    let mut two_moving = moving_source;
    two_moving[2].1 = 0.0;
    let mut two_targets = target;
    two_targets[2].1 = 30.0;
    assert!(
        validate_single_moving_flat_chain_schedule_v1(
            &hinges,
            &two_moving,
            &two_targets,
            FIVE_HINGE_COUNT_V1,
            FIVE_HINGE_COUNT_V1,
        )
        .expect("bounded schedule")
        .is_none()
    );
}

#[test]
fn five_face_limits_and_control_are_bounded() {
    let limits = LayeredFiveFaceChainContinuousLimitsV1::default();
    assert_eq!(limits.interval_limits.max_faces, FIVE_FACE_COUNT_V1);
    assert_eq!(limits.interval_limits.max_hinges, FIVE_HINGE_COUNT_V1);
    assert_eq!(limits.max_nonadjacent_pairs, FIVE_NONADJACENT_PAIR_COUNT_V1);
    assert_eq!(layered_leaf_count_v1(0, 1), Some(1));

    let cancelled = AtomicBool::new(true);
    assert_eq!(
        certificate_checkpoint_v1(&CooperativeOperationControlV1::new(
            Some(&cancelled),
            Instant::now() + Duration::from_secs(1),
        )),
        Err(LayeredFiveFaceChainContinuousErrorV1::Cancelled)
    );
    assert_eq!(
        certificate_checkpoint_v1(&CooperativeOperationControlV1::new(None, Instant::now(),)),
        Err(LayeredFiveFaceChainContinuousErrorV1::DeadlineExceeded)
    );
}

#[test]
fn five_face_certificate_issues_and_revalidates_all_ten_pairs() {
    let fixture = fixture_v1();
    let certificate = certify_layered_five_face_chain_continuous_path_with_control_v1(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        fixture.limits,
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("five-face chain certificate");
    assert_eq!(
        certificate.model_id(),
        LAYERED_FIVE_FACE_CHAIN_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
    );
    assert_eq!(certificate.moving_hinge(), fixture.moving_hinge);
    let mut pairs = certificate.pair_partition();
    pairs.sort_unstable_by_key(pair_key_v1);
    assert!(pairs.windows(2).all(|pairs| pairs[0] != pairs[1]));
    assert_eq!(pairs.len(), FIVE_ALL_PAIR_COUNT_V1);
    assert!(!certificate.authorizes_project_mutation());
    assert_eq!(
        certificate.is_for_with_control_v1(
            &fixture.model,
            &fixture.source_pose,
            &fixture.target_angles,
            &fixture.admission,
            fixture.limits,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Ok(true)
    );
}

#[test]
fn five_face_chain_rejects_malformed_topology_matrix() {
    let (faces, hinges) = canonical_chain_v1();
    assert!(
        validate_linear_chain_hinges_v1(
            &faces[..4],
            &hinges,
            FIVE_FACE_COUNT_V1,
            FIVE_ALL_PAIR_COUNT_V1,
        )
        .expect("bounded topology")
        .is_none()
    );
    assert!(
        validate_linear_chain_hinges_v1(
            &faces,
            &hinges[..3],
            FIVE_FACE_COUNT_V1,
            FIVE_ALL_PAIR_COUNT_V1,
        )
        .expect("bounded topology")
        .is_none()
    );
    let mut reversed = hinges;
    reversed[0].1.swap(0, 1);
    assert!(
        validate_linear_chain_hinges_v1(
            &faces,
            &reversed,
            FIVE_FACE_COUNT_V1,
            FIVE_ALL_PAIR_COUNT_V1,
        )
        .expect("bounded topology")
        .is_none()
    );

    let mut duplicate_edge = hinges;
    duplicate_edge[1].0 = duplicate_edge[0].0;
    assert!(
        validate_linear_chain_hinges_v1(
            &faces,
            &duplicate_edge,
            FIVE_FACE_COUNT_V1,
            FIVE_ALL_PAIR_COUNT_V1,
        )
        .expect("bounded topology")
        .is_none()
    );
    let mut duplicate_pair = hinges;
    duplicate_pair[1].1 = duplicate_pair[0].1;
    assert!(
        validate_linear_chain_hinges_v1(
            &faces,
            &duplicate_pair,
            FIVE_FACE_COUNT_V1,
            FIVE_ALL_PAIR_COUNT_V1,
        )
        .expect("bounded topology")
        .is_none()
    );
    let mut duplicate_face = faces;
    duplicate_face[4] = duplicate_face[3];
    assert!(
        validate_linear_chain_hinges_v1(
            &duplicate_face,
            &hinges,
            FIVE_FACE_COUNT_V1,
            FIVE_ALL_PAIR_COUNT_V1,
        )
        .expect("bounded topology")
        .is_none()
    );

    let star: [(EdgeId, [FaceId; 2]); FIVE_HINGE_COUNT_V1] =
        std::array::from_fn(|index| (EdgeId::new(), canonical_pair_v1(faces[0], faces[index + 1])));
    assert!(
        validate_linear_chain_hinges_v1(&faces, &star, FIVE_FACE_COUNT_V1, FIVE_ALL_PAIR_COUNT_V1,)
            .expect("bounded topology")
            .is_none()
    );
    let disconnected = [
        (EdgeId::new(), canonical_pair_v1(faces[0], faces[1])),
        (EdgeId::new(), canonical_pair_v1(faces[1], faces[2])),
        (EdgeId::new(), canonical_pair_v1(faces[0], faces[2])),
        (EdgeId::new(), canonical_pair_v1(faces[3], faces[4])),
    ];
    assert!(
        validate_linear_chain_hinges_v1(
            &faces,
            &disconnected,
            FIVE_FACE_COUNT_V1,
            FIVE_ALL_PAIR_COUNT_V1,
        )
        .expect("bounded topology")
        .is_none()
    );
}

#[test]
fn five_face_chain_rejects_malformed_schedule_matrix() {
    let (_, hinges) = canonical_chain_v1();
    let mut source = hinges.map(|(edge, _)| (edge, 180.0));
    source[0].1 = 0.0;
    let mut target = source;
    target[0].1 = 45.0;

    for (bad_source, bad_target) in [
        {
            let mut values = source;
            values[1].1 = 0.0;
            let mut targets = target;
            targets[1].1 = 30.0;
            (values, targets)
        },
        {
            let mut values = source;
            values[2].1 = f64::from_bits(180.0_f64.to_bits() - 1);
            (values, target)
        },
        {
            let mut targets = target;
            targets[0].1 = 180.0;
            (source, targets)
        },
        {
            let mut targets = target;
            targets[0].1 = f64::NAN;
            (source, targets)
        },
        {
            let mut values = source;
            values[1].0 = values[0].0;
            (values, target)
        },
        {
            let mut targets = target;
            targets[1].0 = targets[0].0;
            (source, targets)
        },
        {
            let mut targets = target;
            targets[1].0 = EdgeId::new();
            (source, targets)
        },
    ] {
        assert!(
            validate_single_moving_flat_chain_schedule_v1(
                &hinges,
                &bad_source,
                &bad_target,
                FIVE_HINGE_COUNT_V1,
                FIVE_HINGE_COUNT_V1,
            )
            .expect("bounded schedule")
            .is_none()
        );
    }
    assert!(
        validate_single_moving_flat_chain_schedule_v1(
            &hinges,
            &source[..3],
            &target,
            FIVE_HINGE_COUNT_V1,
            FIVE_HINGE_COUNT_V1,
        )
        .expect("bounded schedule")
        .is_none()
    );
}

fn interval_v1(x: (f64, f64)) -> (VertexId, [OutwardIntervalV1; 3]) {
    (
        VertexId::new(),
        [
            OutwardIntervalV1::new(x.0, x.1).expect("outward interval"),
            OutwardIntervalV1::new(0.0, 0.0).expect("point interval"),
            OutwardIntervalV1::new(0.0, 0.0).expect("point interval"),
        ],
    )
}

#[test]
fn six_nonadjacent_gaps_reject_touch_overlap_missing_caps_and_honor_stops() {
    let first = [interval_v1((0.0, 1.0))];
    let separated = [interval_v1((2.0, 3.0))];
    let touching = [interval_v1((1.0, 2.0))];
    let overlap = [interval_v1((0.5, 2.0))];
    let good: [LayeredChainNonadjacentIntervalPairV1<'_>; 6] =
        std::array::from_fn(|_| (&first[..], &separated[..]));
    assert_eq!(
        verify_layered_chain_nonadjacent_gaps_with_control_v1(
            &good,
            6,
            6,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Ok(())
    );
    for blocked in [&touching[..], &overlap[..]] {
        let pairs: [LayeredChainNonadjacentIntervalPairV1<'_>; 6] = std::array::from_fn(|index| {
            if index == 2 {
                (&first[..], blocked)
            } else {
                (&first[..], &separated[..])
            }
        });
        assert_eq!(
            verify_layered_chain_nonadjacent_gaps_with_control_v1(
                &pairs,
                6,
                6,
                &CooperativeOperationControlV1::unbounded(),
            ),
            Err(LayeredChainIntervalErrorV1::IntervalOverlap)
        );
    }
    assert_eq!(
        verify_layered_chain_nonadjacent_gaps_with_control_v1(
            &good,
            6,
            5,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Err(LayeredChainIntervalErrorV1::ResourceLimit)
    );
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        verify_layered_chain_nonadjacent_gaps_with_control_v1(
            &good,
            6,
            6,
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(LayeredChainIntervalErrorV1::Cancelled)
    );
    assert_eq!(
        verify_layered_chain_nonadjacent_gaps_with_control_v1(
            &good,
            6,
            6,
            &CooperativeOperationControlV1::new(None, Instant::now()),
        ),
        Err(LayeredChainIntervalErrorV1::DeadlineExceeded)
    );

    let fixture = fixture_v1();
    let registry = fixture
        .model
        .prepare_dyadic_face_vertex_intervals_v1(
            &fixture.source_pose,
            &fixture.target_angles,
            fixture.limits.dyadic_depth,
            0,
            fixture.limits.interval_limits,
        )
        .expect("five-face registry");
    let missing = [[FaceId::new(), FaceId::new()]; 6];
    assert_eq!(
        verify_layered_chain_nonadjacent_registry_gaps_with_control_v1(
            &registry,
            &missing,
            6,
            6,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Err(LayeredChainIntervalErrorV1::IntervalUnavailable)
    );
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        verify_layered_chain_nonadjacent_registry_gaps_with_control_v1(
            &registry,
            &missing,
            6,
            6,
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(LayeredChainIntervalErrorV1::Cancelled)
    );
    assert_eq!(
        verify_layered_chain_nonadjacent_registry_gaps_with_control_v1(
            &registry,
            &missing,
            6,
            6,
            &CooperativeOperationControlV1::new(None, Instant::now()),
        ),
        Err(LayeredChainIntervalErrorV1::DeadlineExceeded)
    );
}

#[test]
fn five_face_certificate_rejects_every_binding_drift() {
    let fixture = fixture_v1();
    let certificate = certify_layered_five_face_chain_continuous_path_v1(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        fixture.limits,
    )
    .expect("five-face certificate");
    let (_, alternate_target) =
        schedule_for_moving_hinge_v1(&fixture.model, fixture.moving_hinge, 30.0);
    assert!(!certificate.is_for(
        &fixture.model,
        &fixture.source_pose,
        &alternate_target,
        &fixture.admission,
        fixture.limits,
    ));
    let (_, one_bit_target) = schedule_for_moving_hinge_v1(
        &fixture.model,
        fixture.moving_hinge,
        f64::from_bits(90.0_f64.to_bits() - 1),
    );
    assert!(!certificate.is_for(
        &fixture.model,
        &fixture.source_pose,
        &one_bit_target,
        &fixture.admission,
        fixture.limits,
    ));
    let same_value_new_pose = fixture
        .model
        .solve(fixture.source_pose.fixed_face(), &fixture.source_angles)
        .expect("same-valued new pose");
    assert!(!certificate.is_for(
        &fixture.model,
        &same_value_new_pose,
        &fixture.target_angles,
        &fixture.admission,
        fixture.limits,
    ));
    let source = admission_source_v1(&fixture.model, &fixture.source_pose);
    let alternate_admission = prepare_stacked_fold_initial_sample_layer_admission_with_control_v1(
        &fixture.model,
        &fixture.source_pose,
        0.0,
        StaticCollisionLimits::default(),
        &source,
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("alternate issuer");
    assert!(!certificate.is_for(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &alternate_admission,
        fixture.limits,
    ));
    assert!(!certificate.is_for(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        LayeredFiveFaceChainContinuousLimitsV1 {
            max_nonadjacent_pairs: 5,
            ..fixture.limits
        },
    ));
}

#[test]
fn five_face_issuer_rejects_one_bit_stationary_drift() {
    let fixture = fixture_v1();
    let mut angles = fixture.target_angles.as_slice().to_vec();
    let stationary = angles
        .iter_mut()
        .find(|angle| angle.edge() != fixture.moving_hinge)
        .expect("stationary hinge");
    *stationary = HingeAngle::new(stationary.edge(), f64::from_bits(180.0_f64.to_bits() - 1))
        .expect("one-bit stationary drift");
    let drifted = CanonicalHingeAngles::new(angles).expect("canonical drift");
    assert_eq!(
        certify_layered_five_face_chain_continuous_path_v1(
            &fixture.model,
            &fixture.source_pose,
            &drifted,
            &fixture.admission,
            fixture.limits,
        )
        .unwrap_err(),
        LayeredFiveFaceChainContinuousErrorV1::InvalidAngleSchedule
    );
}

#[test]
fn five_face_issuer_and_revalidation_enforce_exact_caps_and_leaf_accounting() {
    let fixture = fixture_v1();
    for invalid in [
        LayeredFiveFaceChainContinuousLimitsV1 {
            max_leaves: (1_usize << fixture.limits.dyadic_depth) - 1,
            ..fixture.limits
        },
        LayeredFiveFaceChainContinuousLimitsV1 {
            max_nonadjacent_pairs: 5,
            ..fixture.limits
        },
        LayeredFiveFaceChainContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_faces: 4,
                ..fixture.limits.interval_limits
            },
            ..fixture.limits
        },
        LayeredFiveFaceChainContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_hinges: 3,
                ..fixture.limits.interval_limits
            },
            ..fixture.limits
        },
    ] {
        assert_eq!(
            certify_layered_five_face_chain_continuous_path_v1(
                &fixture.model,
                &fixture.source_pose,
                &fixture.target_angles,
                &fixture.admission,
                invalid,
            )
            .unwrap_err(),
            LayeredFiveFaceChainContinuousErrorV1::ResourceLimit
        );
    }
    let exact_caps = LayeredFiveFaceChainContinuousLimitsV1 {
        max_leaves: super::super::MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1,
        interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
            max_faces: FIVE_FACE_COUNT_V1,
            max_hinges: FIVE_HINGE_COUNT_V1,
            max_vertices:
                super::super::layered_chain_common::MAX_LAYERED_CONTINUOUS_INTERVAL_VERTICES_V1,
            max_interval_work:
                super::super::layered_chain_common::MAX_LAYERED_CONTINUOUS_INTERVAL_WORK_V1,
            max_total_interval_work:
                super::super::layered_chain_common::MAX_LAYERED_CONTINUOUS_TOTAL_INTERVAL_WORK_V1,
        },
        static_limits:
            super::super::layered_chain_common::LAYERED_CONTINUOUS_STATIC_LIMIT_HARD_CAPS_V1,
        ..fixture.limits
    };
    let certificate = certify_layered_five_face_chain_continuous_path_v1(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        exact_caps,
    )
    .expect("exact caps issue");
    assert!(certificate.is_for(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        exact_caps,
    ));
}

#[test]
fn five_face_control_and_issuer_mismatch_fail_closed() {
    let fixture = fixture_v1();
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        certify_layered_five_face_chain_continuous_path_with_control_v1(
            &fixture.model,
            &fixture.source_pose,
            &fixture.target_angles,
            &fixture.admission,
            fixture.limits,
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        )
        .unwrap_err(),
        LayeredFiveFaceChainContinuousErrorV1::Cancelled
    );
    assert_eq!(
        certify_layered_five_face_chain_continuous_path_with_control_v1(
            &fixture.model,
            &fixture.source_pose,
            &fixture.target_angles,
            &fixture.admission,
            fixture.limits,
            &CooperativeOperationControlV1::new(None, Instant::now()),
        )
        .unwrap_err(),
        LayeredFiveFaceChainContinuousErrorV1::DeadlineExceeded
    );
    let certificate = certify_layered_five_face_chain_continuous_path_v1(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        fixture.limits,
    )
    .expect("five-face certificate");
    assert_eq!(
        certificate.is_for_with_control_v1(
            &fixture.model,
            &fixture.source_pose,
            &fixture.target_angles,
            &fixture.admission,
            fixture.limits,
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(LayeredFiveFaceChainContinuousErrorV1::Cancelled)
    );
    assert_eq!(
        certificate.is_for_with_control_v1(
            &fixture.model,
            &fixture.source_pose,
            &fixture.target_angles,
            &fixture.admission,
            fixture.limits,
            &CooperativeOperationControlV1::new(None, Instant::now()),
        ),
        Err(LayeredFiveFaceChainContinuousErrorV1::DeadlineExceeded)
    );
    let other_model = five_face_four_hinge_model_v1(2);
    let (other_source_angles, other_target_angles) =
        schedule_for_moving_hinge_v1(&other_model, other_model.hinges()[0].edge(), 90.0);
    let other_pose = other_model
        .solve(Some(other_model.face_ids()[0]), &other_source_angles)
        .expect("same-shaped foreign pose");
    assert_eq!(
        certify_layered_five_face_chain_continuous_path_v1(
            &other_model,
            &other_pose,
            &other_target_angles,
            &fixture.admission,
            fixture.limits,
        )
        .unwrap_err(),
        LayeredFiveFaceChainContinuousErrorV1::InitialLayerAdmissionUnavailable
    );
}

#[test]
fn five_face_reversed_transport_is_new_authority_not_old_certificate_authority() {
    let fixture = fixture_v1();
    let certificate = certify_layered_five_face_chain_continuous_path_v1(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        fixture.limits,
    )
    .expect("original certificate");
    let mut reversed_source = admission_source_v1(&fixture.model, &fixture.source_pose);
    for order in &mut reversed_source.orders {
        std::mem::swap(&mut order.lower_face, &mut order.upper_face);
    }
    for (_, _, lower, upper) in &mut reversed_source.cells {
        std::mem::swap(lower, upper);
    }
    let reversed_admission = prepare_stacked_fold_initial_sample_layer_admission_with_control_v1(
        &fixture.model,
        &fixture.source_pose,
        0.0,
        StaticCollisionLimits::default(),
        &reversed_source,
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("reversed acyclic authority");
    assert!(!certificate.is_for(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &reversed_admission,
        fixture.limits,
    ));
    let reversed_certificate = certify_layered_five_face_chain_continuous_path_v1(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &reversed_admission,
        fixture.limits,
    )
    .expect("new certificate binds reversed authority");
    assert!(reversed_certificate.is_for(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &reversed_admission,
        fixture.limits,
    ));
}
