use std::{
    sync::atomic::{AtomicBool, AtomicU64},
    time::{Duration, Instant},
};

use super::*;
use crate::{
    NonFlatFacePairOrderStructuralV1, NonFlatFoldedFaceStructuralRefV1,
    NonFlatLayerOrderStructuralSourceV1, NonFlatOverlapCellStructuralRefV1,
    prepare_stacked_fold_initial_sample_layer_admission_with_control_v1,
};
use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex};
use ori_foldability::{ExactAffineTransform, ExactPointValue, ExactRationalValue, ExactSign};
use ori_kinematics::{
    CanonicalHingeAngles, HingeAngle, MaterialTreeDyadicIntervalLimitsV1,
    MaterialTreeKinematicsModel, MaterialTreePose, TreeKinematicsLimits,
};
use ori_topology::{FaceExtractionInput, analyze_faces};

struct FourFaceInitialLayerSourceV1 {
    faces: Vec<FaceId>,
    transforms: Vec<ExactAffineTransform>,
    cells: Vec<(Vec<Point2>, Vec<ExactPointValue>, FaceId, FaceId)>,
    orders: Vec<NonFlatFacePairOrderStructuralV1>,
    fixed_face: FaceId,
    hinge_angles: Vec<(EdgeId, u64)>,
}

impl NonFlatLayerOrderStructuralSourceV1 for FourFaceInitialLayerSourceV1 {
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
        self.orders.len()
    }

    fn face_pair_order(&self, index: usize) -> Option<NonFlatFacePairOrderStructuralV1> {
        self.orders.get(index).copied()
    }
}

impl StackedFoldInitialLayerOrderSourceV1 for FourFaceInitialLayerSourceV1 {
    fn tested_face_pairs_v1(&self) -> usize {
        6
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

fn four_face_three_hinge_model_v1(namespace: u64) -> MaterialTreeKinematicsModel {
    let points = [
        (0.0, 0.0),
        (500.0, 0.0),
        (1_000.0, 0.0),
        (1_900.0, 0.0),
        (2_400.0, 0.0),
        (2_400.0, 300.0),
        (1_500.0, 300.0),
        (1_400.0, 300.0),
        (100.0, 300.0),
        (0.0, 300.0),
    ];
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_id("b420", namespace * 100 + index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("b421", namespace * 100 + index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend([
        Edge {
            id: fixed_id("b421", namespace * 100 + 11),
            start: boundary[1],
            end: boundary[8],
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: fixed_id("b421", namespace * 100 + 12),
            start: boundary[2],
            end: boundary[7],
            kind: EdgeKind::Valley,
        },
        Edge {
            id: fixed_id("b421", namespace * 100 + 13),
            start: boundary[3],
            end: boundary[6],
            kind: EdgeKind::Mountain,
        },
    ]);
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: fixed_id::<ProjectId>("b422", namespace),
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

fn schedule_for_moving_hinge_v1(
    model: &MaterialTreeKinematicsModel,
    moving_hinge: EdgeId,
    moving_target: f64,
) -> (CanonicalHingeAngles, CanonicalHingeAngles) {
    let source = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| {
                HingeAngle::new(
                    hinge.edge(),
                    if hinge.edge() == moving_hinge {
                        0.0
                    } else {
                        180.0
                    },
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    let target = CanonicalHingeAngles::new(
        model
            .hinges()
            .iter()
            .map(|hinge| {
                HingeAngle::new(
                    hinge.edge(),
                    if hinge.edge() == moving_hinge {
                        moving_target
                    } else {
                        180.0
                    },
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    (source, target)
}

fn production_source_v1(
    model: &MaterialTreeKinematicsModel,
) -> (EdgeId, CanonicalHingeAngles, MaterialTreePose, u8) {
    let mut observed = Vec::new();
    let direct_hinges = model
        .hinges()
        .iter()
        .map(|hinge| {
            (
                hinge.edge(),
                canonical_pair_v1(hinge.left_face(), hinge.right_face()),
            )
        })
        .collect::<Vec<_>>();
    let pair_partition =
        validate_four_face_chain_hinges_v1(model.face_ids(), &direct_hinges).unwrap();
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
            let moving_boundary_theorem = direct_hinge_boundary_only_open_interval_theorem_v1(
                model,
                &source_pose,
                &target_pose,
                moving_hinge,
                moving_pair,
                &CooperativeOperationControlV1::unbounded(),
            )
            .unwrap_or(false);
            let target_pair = diagnose_static_collision_geometry_with_control_v1(
                model,
                &target_pose,
                0.0,
                StaticCollisionLimits::default(),
                &CooperativeOperationControlV1::unbounded(),
            )
            .ok()
            .and_then(|target| {
                target
                    .pairs()
                    .iter()
                    .find(|pair| {
                        canonical_pair_v1(pair.first_face(), pair.second_face()) == moving_pair
                    })
                    .cloned()
            });
            let flat_pairs = snapshot
                .pairs()
                .iter()
                .filter(|pair| {
                    pair.disposition() == crate::StaticCollisionPairDisposition::Indeterminate
                        && pair.evidence() == crate::IntersectionEvidenceV2::SharedFeatureFlatStack
                })
                .collect::<Vec<_>>();
            let exact_stationary_flat_pairs = flat_pairs.len() == 2
                && flat_pairs.iter().all(|pair| {
                    stationary_pairs
                        .contains(&canonical_pair_v1(pair.first_face(), pair.second_face()))
                });
            let exact_moving_boundary_pair = snapshot.pairs().iter().any(|pair| {
                canonical_pair_v1(pair.first_face(), pair.second_face()) == moving_pair
                    && pair.disposition() == crate::StaticCollisionPairDisposition::Allowed
                    && pair.evidence() == crate::IntersectionEvidenceV2::SharedFeatureContact
            });
            let gap_depth = if moving_boundary_theorem {
                minimum_nonadjacent_gap_depth_v1(
                    model,
                    &source_pose,
                    &target_angles,
                    pair_partition.nonadjacent_pairs,
                )
            } else {
                None
            };
            if snapshot.penetrating_pairs() == 0
                && snapshot.candidate_excluded_pairs() == 0
                && snapshot.indeterminate_pairs() == 2
                && snapshot.touching_pairs() == 0
                && snapshot.allowed_pairs() == 1
                && snapshot.separated_pairs() == 3
                && exact_stationary_flat_pairs
                && exact_moving_boundary_pair
                && moving_boundary_theorem
                && gap_depth.is_some()
            {
                return (moving_hinge, source_angles, source_pose, gap_depth.unwrap());
            }
            observed.push((
                moving_hinge,
                *fixed_face,
                moving_boundary_theorem,
                gap_depth,
                target_pair,
                snapshot.pairs().to_vec(),
            ));
        }
    }
    panic!("no production-valid four-face source candidate: {observed:?}");
}

fn minimum_nonadjacent_gap_depth_v1(
    model: &MaterialTreeKinematicsModel,
    source_pose: &MaterialTreePose,
    target_angles: &CanonicalHingeAngles,
    pairs: [[FaceId; 2]; 3],
) -> Option<u8> {
    let interval_limits = LayeredFourFaceChainContinuousLimitsV1::default().interval_limits;
    let maximum_depth =
        u8::try_from(super::super::MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1.ilog2()).ok()?;
    for depth in 0_u8..=maximum_depth {
        let leaf_count = 1_usize << depth;
        let mut complete = true;
        for index in 0..leaf_count {
            let Ok(registry) = model.prepare_dyadic_face_vertex_intervals_v1(
                source_pose,
                target_angles,
                depth,
                index as u64,
                interval_limits,
            ) else {
                complete = false;
                break;
            };
            if verify_four_face_chain_nonadjacent_registry_gaps_with_control_v1(
                &registry,
                &pairs,
                3,
                &CooperativeOperationControlV1::unbounded(),
            )
            .is_err()
            {
                complete = false;
                break;
            }
        }
        if complete {
            return Some(depth);
        }
    }
    None
}

#[test]
fn missing_four_face_registry_pair_yields_to_typed_stop_only_when_stopped() {
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
        .expect("bounded four-face registry");
    let missing_pair = [FaceId::new(), FaceId::new()];
    let missing_pairs = [missing_pair; 3];
    assert_eq!(
        verify_four_face_chain_nonadjacent_registry_gaps_with_control_v1(
            &registry,
            &missing_pairs,
            3,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Err(FourFaceChainIntervalErrorV1::IntervalUnavailable),
        "an unstopped missing registry pair remains unavailable"
    );

    let cancelled = AtomicBool::new(true);
    assert_eq!(
        verify_four_face_chain_nonadjacent_registry_gaps_with_control_v1(
            &registry,
            &missing_pairs,
            3,
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(FourFaceChainIntervalErrorV1::Cancelled)
    );
    assert_eq!(
        verify_four_face_chain_nonadjacent_registry_gaps_with_control_v1(
            &registry,
            &missing_pairs,
            3,
            &CooperativeOperationControlV1::new(None, Instant::now()),
        ),
        Err(FourFaceChainIntervalErrorV1::DeadlineExceeded)
    );
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
) -> FourFaceInitialLayerSourceV1 {
    let snapshot = diagnose_static_collision_geometry_with_control_v1(
        model,
        pose,
        0.0,
        StaticCollisionLimits::default(),
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("four-face source pose static diagnostic");
    assert_eq!(
        snapshot.penetrating_pairs(),
        0,
        "fixture has a penetrating pair: {:?}",
        snapshot.pairs()
    );
    assert_eq!(
        snapshot.candidate_excluded_pairs(),
        0,
        "fixture has an excluded pair: {:?}",
        snapshot.pairs()
    );
    assert!(
        snapshot
            .pairs()
            .iter()
            .filter(|pair| {
                pair.disposition() == crate::StaticCollisionPairDisposition::Indeterminate
            })
            .all(|pair| pair.evidence() == crate::IntersectionEvidenceV2::SharedFeatureFlatStack)
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
    assert_eq!(
        orders.len(),
        2,
        "fixture must have exactly the two stationary flat-stack pairs: {:?}",
        snapshot.pairs()
    );
    let cells = orders
        .iter()
        .map(|order| {
            let (boundary, exact_boundary) = exact_triangle_v1();
            (boundary, exact_boundary, order.lower_face, order.upper_face)
        })
        .collect();
    FourFaceInitialLayerSourceV1 {
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
    }
}

#[derive(Clone)]
struct FourFaceFixtureV1 {
    model: MaterialTreeKinematicsModel,
    moving_hinge: EdgeId,
    source_angles: CanonicalHingeAngles,
    source_pose: MaterialTreePose,
    target_angles: CanonicalHingeAngles,
    admission: NativeStackedFoldInitialSampleLayerAdmissionV1<FourFaceInitialLayerSourceV1>,
    limits: LayeredFourFaceChainContinuousLimitsV1,
}

fn fixture_v1() -> FourFaceFixtureV1 {
    static FIXTURE: std::sync::OnceLock<FourFaceFixtureV1> = std::sync::OnceLock::new();
    FIXTURE.get_or_init(build_fixture_v1).clone()
}

fn build_fixture_v1() -> FourFaceFixtureV1 {
    let model = four_face_three_hinge_model_v1(1);
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
    .expect("four-face initial layer admission");
    FourFaceFixtureV1 {
        model,
        moving_hinge,
        source_angles,
        source_pose,
        target_angles,
        admission,
        limits: LayeredFourFaceChainContinuousLimitsV1 {
            dyadic_depth,
            max_leaves: 1_usize << dyadic_depth,
            ..LayeredFourFaceChainContinuousLimitsV1::default()
        },
    }
}

fn minimum_passing_limit_v1(upper_bound: usize, mut passes: impl FnMut(usize) -> bool) -> usize {
    assert!(passes(upper_bound), "the configured upper bound must pass");
    let mut lower = 0_usize;
    let mut upper = upper_bound;
    while lower < upper {
        let candidate = lower + (upper - lower) / 2;
        if passes(candidate) {
            upper = candidate;
        } else {
            lower = candidate + 1;
        }
    }
    lower
}

#[test]
fn four_face_chain_certificate_issues_and_revalidates_all_six_pairs() {
    let fixture = fixture_v1();
    let certificate = certify_layered_four_face_chain_continuous_path_with_control_v1(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        fixture.limits,
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("four-face chain certificate");
    assert_eq!(
        certificate.model_id(),
        LAYERED_FOUR_FACE_CHAIN_CONTINUOUS_CERTIFICATE_MODEL_ID_V1
    );
    assert_eq!(certificate.moving_hinge(), fixture.moving_hinge);
    let mut pairs = certificate.pair_partition();
    pairs.sort_unstable_by_key(pair_key_v1);
    assert!(pairs.windows(2).all(|window| window[0] != window[1]));
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
fn four_face_chain_certificate_rejects_every_drifted_binding() {
    let fixture = fixture_v1();
    let certificate = certify_layered_four_face_chain_continuous_path_v1(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        fixture.limits,
    )
    .unwrap();

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

    let same_value_distinct_source = fixture
        .model
        .solve(fixture.source_pose.fixed_face(), &fixture.source_angles)
        .expect("same-value independently issued source pose");
    assert!(!certificate.is_for(
        &fixture.model,
        &same_value_distinct_source,
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
    .unwrap();
    assert!(!certificate.is_for(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &alternate_admission,
        fixture.limits,
    ));

    let changed_limits = LayeredFourFaceChainContinuousLimitsV1 {
        max_nonadjacent_pairs: 4,
        ..fixture.limits
    };
    assert!(!certificate.is_for(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        changed_limits,
    ));

    let target_pose = fixture
        .model
        .solve(fixture.source_pose.fixed_face(), &fixture.target_angles)
        .unwrap();
    assert!(!certificate.is_for(
        &fixture.model,
        &target_pose,
        &fixture.target_angles,
        &fixture.admission,
        fixture.limits,
    ));

    let other_model = four_face_three_hinge_model_v1(1);
    let other_source_pose = other_model
        .solve(fixture.source_pose.fixed_face(), &fixture.source_angles)
        .expect("same-shaped independent model source");
    assert!(!certificate.is_for(
        &other_model,
        &other_source_pose,
        &fixture.target_angles,
        &fixture.admission,
        fixture.limits,
    ));
}

#[test]
fn four_face_chain_certificate_has_fail_closed_resource_limits() {
    let fixture = fixture_v1();
    for limits in [
        LayeredFourFaceChainContinuousLimitsV1 {
            max_leaves: (1_usize << fixture.limits.dyadic_depth) - 1,
            ..fixture.limits
        },
        LayeredFourFaceChainContinuousLimitsV1 {
            max_nonadjacent_pairs: 2,
            ..fixture.limits
        },
        LayeredFourFaceChainContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_faces: 3,
                ..fixture.limits.interval_limits
            },
            ..fixture.limits
        },
        LayeredFourFaceChainContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_hinges: 2,
                ..fixture.limits.interval_limits
            },
            ..fixture.limits
        },
    ] {
        assert_eq!(
            certify_layered_four_face_chain_continuous_path_v1(
                &fixture.model,
                &fixture.source_pose,
                &fixture.target_angles,
                &fixture.admission,
                limits,
            )
            .unwrap_err(),
            LayeredFourFaceChainContinuousErrorV1::ResourceLimit
        );
    }
}

#[test]
fn four_face_chain_issuer_and_revalidation_enforce_layered_hard_caps() {
    let fixture = fixture_v1();
    let exact_caps = LayeredFourFaceChainContinuousLimitsV1 {
        max_leaves: super::super::MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1,
        max_nonadjacent_pairs: 3,
        interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
            max_faces: 4,
            max_hinges: 3,
            max_vertices:
                super::super::layered_three_face::MAX_LAYERED_CONTINUOUS_INTERVAL_VERTICES_V1,
            max_interval_work:
                super::super::layered_three_face::MAX_LAYERED_CONTINUOUS_INTERVAL_WORK_V1,
            max_total_interval_work:
                super::super::layered_three_face::MAX_LAYERED_CONTINUOUS_TOTAL_INTERVAL_WORK_V1,
        },
        static_limits:
            super::super::layered_three_face::LAYERED_CONTINUOUS_STATIC_LIMIT_HARD_CAPS_V1,
        ..fixture.limits
    };
    let certificate = certify_layered_four_face_chain_continuous_path_v1(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        exact_caps,
    )
    .expect("the exact layered hard caps must issue");
    assert_eq!(
        certificate.is_for_with_control_v1(
            &fixture.model,
            &fixture.source_pose,
            &fixture.target_angles,
            &fixture.admission,
            exact_caps,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Ok(true)
    );

    for one_over in [
        LayeredFourFaceChainContinuousLimitsV1 {
            max_leaves: exact_caps.max_leaves + 1,
            ..exact_caps
        },
        LayeredFourFaceChainContinuousLimitsV1 {
            max_nonadjacent_pairs: exact_caps.max_nonadjacent_pairs + 1,
            ..exact_caps
        },
        LayeredFourFaceChainContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_faces: exact_caps.interval_limits.max_faces + 1,
                ..exact_caps.interval_limits
            },
            ..exact_caps
        },
        LayeredFourFaceChainContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_hinges: exact_caps.interval_limits.max_hinges + 1,
                ..exact_caps.interval_limits
            },
            ..exact_caps
        },
        LayeredFourFaceChainContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_vertices: exact_caps.interval_limits.max_vertices + 1,
                ..exact_caps.interval_limits
            },
            ..exact_caps
        },
        LayeredFourFaceChainContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_interval_work: exact_caps.interval_limits.max_interval_work + 1,
                ..exact_caps.interval_limits
            },
            ..exact_caps
        },
        LayeredFourFaceChainContinuousLimitsV1 {
            interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
                max_total_interval_work: exact_caps.interval_limits.max_total_interval_work + 1,
                ..exact_caps.interval_limits
            },
            ..exact_caps
        },
        LayeredFourFaceChainContinuousLimitsV1 {
            static_limits: StaticCollisionLimits {
                max_total_rational_allocation_bits: exact_caps
                    .static_limits
                    .max_total_rational_allocation_bits
                    + 1,
                ..exact_caps.static_limits
            },
            ..exact_caps
        },
    ] {
        assert_eq!(
            certify_layered_four_face_chain_continuous_path_v1(
                &fixture.model,
                &fixture.source_pose,
                &fixture.target_angles,
                &fixture.admission,
                one_over,
            )
            .unwrap_err(),
            LayeredFourFaceChainContinuousErrorV1::ResourceLimit
        );
        assert_eq!(
            certificate.is_for_with_control_v1(
                &fixture.model,
                &fixture.source_pose,
                &fixture.target_angles,
                &fixture.admission,
                one_over,
                &CooperativeOperationControlV1::unbounded(),
            ),
            Ok(false)
        );
    }
}

#[test]
fn four_face_chain_certificate_binds_exact_interval_and_total_leaf_work() {
    let fixture = fixture_v1();
    let template = fixture.limits.interval_limits;
    let leaf_count = 1_usize << fixture.limits.dyadic_depth;
    let minimum_interval_work =
        minimum_passing_limit_v1(template.max_interval_work, |max_interval_work| {
            (0..leaf_count).all(|index| {
                fixture
                    .model
                    .prepare_dyadic_face_vertex_intervals_v1(
                        &fixture.source_pose,
                        &fixture.target_angles,
                        fixture.limits.dyadic_depth,
                        index as u64,
                        MaterialTreeDyadicIntervalLimitsV1 {
                            max_interval_work,
                            ..template
                        },
                    )
                    .is_ok()
            })
        });
    let minimum_total_work = minimum_passing_limit_v1(
        template.max_total_interval_work,
        |max_total_interval_work| {
            (0..leaf_count).all(|index| {
                fixture
                    .model
                    .prepare_dyadic_face_vertex_intervals_v1(
                        &fixture.source_pose,
                        &fixture.target_angles,
                        fixture.limits.dyadic_depth,
                        index as u64,
                        MaterialTreeDyadicIntervalLimitsV1 {
                            max_total_interval_work,
                            ..template
                        },
                    )
                    .is_ok()
            })
        },
    );
    assert!(minimum_interval_work > 0);
    assert!(minimum_total_work > 0);

    let exact_limits = LayeredFourFaceChainContinuousLimitsV1 {
        interval_limits: MaterialTreeDyadicIntervalLimitsV1 {
            max_interval_work: minimum_interval_work,
            max_total_interval_work: minimum_total_work,
            ..template
        },
        ..fixture.limits
    };
    certify_layered_four_face_chain_continuous_path_v1(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        exact_limits,
    )
    .expect("the exact per-value and total interval work must issue");

    for interval_limits in [
        MaterialTreeDyadicIntervalLimitsV1 {
            max_interval_work: minimum_interval_work - 1,
            max_total_interval_work: minimum_total_work,
            ..template
        },
        MaterialTreeDyadicIntervalLimitsV1 {
            max_interval_work: minimum_interval_work,
            max_total_interval_work: minimum_total_work - 1,
            ..template
        },
    ] {
        assert_eq!(
            certify_layered_four_face_chain_continuous_path_v1(
                &fixture.model,
                &fixture.source_pose,
                &fixture.target_angles,
                &fixture.admission,
                LayeredFourFaceChainContinuousLimitsV1 {
                    interval_limits,
                    ..exact_limits
                },
            )
            .unwrap_err(),
            LayeredFourFaceChainContinuousErrorV1::ResourceLimit
        );
    }
}

#[test]
fn four_face_chain_certificate_distinguishes_cancel_deadline_and_generation_stop() {
    let fixture = fixture_v1();
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        certify_layered_four_face_chain_continuous_path_with_control_v1(
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
        LayeredFourFaceChainContinuousErrorV1::Cancelled
    );

    assert_eq!(
        certify_layered_four_face_chain_continuous_path_with_control_v1(
            &fixture.model,
            &fixture.source_pose,
            &fixture.target_angles,
            &fixture.admission,
            fixture.limits,
            &CooperativeOperationControlV1::new(None, Instant::now()),
        )
        .unwrap_err(),
        LayeredFourFaceChainContinuousErrorV1::DeadlineExceeded
    );

    let generation = AtomicU64::new(2);
    assert_eq!(
        certify_layered_four_face_chain_continuous_path_with_control_v1(
            &fixture.model,
            &fixture.source_pose,
            &fixture.target_angles,
            &fixture.admission,
            fixture.limits,
            &CooperativeOperationControlV1::new_with_generation(
                None,
                &generation,
                1,
                Instant::now() + Duration::from_secs(1),
            ),
        )
        .unwrap_err(),
        LayeredFourFaceChainContinuousErrorV1::Cancelled
    );

    let certificate = certify_layered_four_face_chain_continuous_path_v1(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        fixture.limits,
    )
    .unwrap();
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
        Err(LayeredFourFaceChainContinuousErrorV1::Cancelled)
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
        Err(LayeredFourFaceChainContinuousErrorV1::DeadlineExceeded)
    );
}

#[test]
fn four_face_chain_rejects_a_one_bit_drifted_stationary_schedule_before_issuance() {
    let fixture = fixture_v1();
    let moving_hinge = fixture.moving_hinge;
    let stationary_to_drift = fixture
        .model
        .hinges()
        .iter()
        .find(|hinge| hinge.edge() != moving_hinge)
        .unwrap()
        .edge();
    let invalid_target = CanonicalHingeAngles::new(
        fixture
            .model
            .hinges()
            .iter()
            .map(|hinge| {
                let angle = if hinge.edge() == moving_hinge {
                    90.0
                } else if hinge.edge() == stationary_to_drift {
                    f64::from_bits(180.0_f64.to_bits() - 1)
                } else {
                    180.0
                };
                HingeAngle::new(hinge.edge(), angle).unwrap()
            })
            .collect(),
    )
    .unwrap();
    assert_eq!(
        certify_layered_four_face_chain_continuous_path_v1(
            &fixture.model,
            &fixture.source_pose,
            &invalid_target,
            &fixture.admission,
            fixture.limits,
        )
        .unwrap_err(),
        LayeredFourFaceChainContinuousErrorV1::InvalidAngleSchedule
    );
}

#[test]
fn four_face_chain_rejects_an_admission_from_an_independent_pose_issuer() {
    let fixture = fixture_v1();
    let other_model = four_face_three_hinge_model_v1(1);
    let other_source_pose = other_model
        .solve(fixture.source_pose.fixed_face(), &fixture.source_angles)
        .unwrap();
    let other_source = admission_source_v1(&other_model, &other_source_pose);
    let other_admission = prepare_stacked_fold_initial_sample_layer_admission_with_control_v1(
        &other_model,
        &other_source_pose,
        0.0,
        StaticCollisionLimits::default(),
        &other_source,
        &CooperativeOperationControlV1::unbounded(),
    )
    .unwrap();
    assert_eq!(
        certify_layered_four_face_chain_continuous_path_v1(
            &fixture.model,
            &fixture.source_pose,
            &fixture.target_angles,
            &other_admission,
            fixture.limits,
        )
        .unwrap_err(),
        LayeredFourFaceChainContinuousErrorV1::InitialLayerAdmissionUnavailable
    );
}

#[test]
fn four_face_chain_transport_rejects_a_reversed_source_order_authority() {
    let fixture = fixture_v1();
    let certificate = certify_layered_four_face_chain_continuous_path_v1(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &fixture.admission,
        fixture.limits,
    )
    .unwrap();

    let mut reversed_source = admission_source_v1(&fixture.model, &fixture.source_pose);
    for order in &mut reversed_source.orders {
        std::mem::swap(&mut order.lower_face, &mut order.upper_face);
    }
    for (_, _, lower_face, upper_face) in &mut reversed_source.cells {
        std::mem::swap(lower_face, upper_face);
    }
    let reversed_admission = prepare_stacked_fold_initial_sample_layer_admission_with_control_v1(
        &fixture.model,
        &fixture.source_pose,
        0.0,
        StaticCollisionLimits::default(),
        &reversed_source,
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("the reversed acyclic source order is independently authentic");

    assert!(!certificate.is_for(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &reversed_admission,
        fixture.limits,
    ));
    let reversed_certificate = certify_layered_four_face_chain_continuous_path_v1(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &reversed_admission,
        fixture.limits,
    )
    .expect("a separately issued certificate may bind the reversed source authority");
    assert!(reversed_certificate.is_for(
        &fixture.model,
        &fixture.source_pose,
        &fixture.target_angles,
        &reversed_admission,
        fixture.limits,
    ));
}
