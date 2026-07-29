use super::*;
use ori_domain::{CreasePattern, Edge, Paper, Point2, Vertex};

fn layered_target_angles_v1(
    requested: &PreparedStackedFoldRequestedPoseV1,
) -> CanonicalHingeAngles {
    CanonicalHingeAngles::new(requested.pose().hinge_angles().to_vec())
        .expect("canonical layered target angles")
}

fn layered_applied_pose_v1(
    requested: &PreparedStackedFoldRequestedPoseV1,
    moving_target_ulp_delta: u64,
) -> AppliedPoseV1 {
    let hinge_ids = requested
        .pose()
        .hinges()
        .iter()
        .map(|hinge| hinge.edge())
        .collect::<Vec<_>>();
    let mut changed_count = 0;
    let hinge_angles = requested
        .pose()
        .hinge_angles()
        .iter()
        .map(|angle| {
            let value = if angle.angle_degrees().to_bits() != 180.0_f64.to_bits() {
                changed_count += 1;
                f64::from_bits(
                    angle
                        .angle_degrees()
                        .to_bits()
                        .checked_add(moving_target_ulp_delta)
                        .expect("finite one-ULP target"),
                )
            } else {
                angle.angle_degrees()
            };
            (angle.edge(), value)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        changed_count, 1,
        "layered fixture has exactly one non-stationary requested hinge"
    );
    prepare_applied_pose_v1(
        requested.pose().face_ids(),
        &hinge_ids,
        requested.pose().fixed_face(),
        &hinge_angles,
        AppliedPoseLimitsV1::default(),
    )
    .expect("layered semantic pose")
}

fn layered_ticket_with_anchor_v1(
    requested: &PreparedStackedFoldRequestedPoseV1,
    editor_instance_anchor: Arc<()>,
) -> SpeculativeUnprovenFoldResolutionTicketV1 {
    let lineage = requested.initial().target().geometry().proof().lineage();
    let paper_thickness_mm = requested
        .initial()
        .target()
        .geometry()
        .candidate()
        .paper
        .thickness_mm;
    let binding = SpeculativeUnprovenFoldBindingV1::new(
        ProjectId::new(),
        lineage.identity_namespace(),
        lineage.source_revision(),
        lineage.source_fingerprint().to_hex(),
        1,
        ProjectId::new(),
        paper_thickness_mm,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    )
    .expect("layered speculative binding");
    SpeculativeUnprovenFoldResolutionTicketV1::new(
        editor_instance_anchor,
        binding,
        lineage.target_revision(),
        lineage.target_fingerprint().0,
        layered_applied_pose_v1(requested, 0),
        Some(PreparedStackedFoldRequestIssuerSealV1::capture(requested)),
    )
}

struct ThreeFaceBinderFixtureV1 {
    requested: PreparedStackedFoldRequestedPoseV1,
    same_semantics_other_requested: PreparedStackedFoldRequestedPoseV1,
    initial_layer_order: StackedFoldInitialLayerOrderV1,
    admission: NativeStackedFoldInitialSampleLayerAdmissionV1<StackedFoldInitialLayerOrderV1>,
    same_semantics_other_admission:
        NativeStackedFoldInitialSampleLayerAdmissionV1<StackedFoldInitialLayerOrderV1>,
    limits: LayeredThreeFaceContinuousLimitsV1,
}

impl ThreeFaceBinderFixtureV1 {
    fn target_angles(&self) -> CanonicalHingeAngles {
        layered_target_angles_v1(&self.requested)
    }

    fn issue_certificate(&self) -> LayeredThreeFaceContinuousCertificateV1 {
        ori_collision::certify_layered_three_face_continuous_path_v1(
            self.requested.initial().target().model(),
            self.requested.initial().pose(),
            &self.target_angles(),
            &self.admission,
            self.limits,
        )
        .expect("reissue the exact three-face certificate")
    }

    fn same_semantics_other_target_angles(&self) -> CanonicalHingeAngles {
        layered_target_angles_v1(&self.same_semantics_other_requested)
    }

    fn issue_same_semantics_other_certificate(&self) -> LayeredThreeFaceContinuousCertificateV1 {
        ori_collision::certify_layered_three_face_continuous_path_v1(
            self.same_semantics_other_requested
                .initial()
                .target()
                .model(),
            self.same_semantics_other_requested.initial().pose(),
            &self.same_semantics_other_target_angles(),
            &self.same_semantics_other_admission,
            self.limits,
        )
        .expect("reissue the semantically equal independent three-face certificate")
    }

    fn alternate_admission(
        &self,
    ) -> NativeStackedFoldInitialSampleLayerAdmissionV1<StackedFoldInitialLayerOrderV1> {
        ori_collision::prepare_stacked_fold_initial_sample_layer_admission_v1(
            self.requested.initial().target().model(),
            self.requested.initial().pose(),
            0.0,
            self.limits.static_limits,
            &self.initial_layer_order,
        )
        .expect("independently issued three-face initial admission")
    }

    fn applied_pose(&self, moving_target_ulp_delta: u64) -> AppliedPoseV1 {
        layered_applied_pose_v1(&self.requested, moving_target_ulp_delta)
    }

    fn ticket_with_anchor(
        &self,
        editor_instance_anchor: Arc<()>,
    ) -> SpeculativeUnprovenFoldResolutionTicketV1 {
        layered_ticket_with_anchor_v1(&self.requested, editor_instance_anchor)
    }

    fn ticket(&self) -> SpeculativeUnprovenFoldResolutionTicketV1 {
        self.ticket_with_anchor(Arc::new(()))
    }
}

struct FourFaceBinderFixtureV1 {
    requested: PreparedStackedFoldRequestedPoseV1,
    same_semantics_other_requested: PreparedStackedFoldRequestedPoseV1,
    initial_layer_order: StackedFoldInitialLayerOrderV1,
    admission: NativeStackedFoldInitialSampleLayerAdmissionV1<StackedFoldInitialLayerOrderV1>,
    same_semantics_other_admission:
        NativeStackedFoldInitialSampleLayerAdmissionV1<StackedFoldInitialLayerOrderV1>,
    limits: LayeredFourFaceChainContinuousLimitsV1,
}

impl FourFaceBinderFixtureV1 {
    fn target_angles(&self) -> CanonicalHingeAngles {
        layered_target_angles_v1(&self.requested)
    }

    fn issue_certificate(&self) -> LayeredFourFaceChainContinuousCertificateV1 {
        ori_collision::certify_layered_four_face_chain_continuous_path_v1(
            self.requested.initial().target().model(),
            self.requested.initial().pose(),
            &self.target_angles(),
            &self.admission,
            self.limits,
        )
        .expect("reissue the exact four-face certificate")
    }

    fn same_semantics_other_target_angles(&self) -> CanonicalHingeAngles {
        layered_target_angles_v1(&self.same_semantics_other_requested)
    }

    fn issue_same_semantics_other_certificate(
        &self,
    ) -> LayeredFourFaceChainContinuousCertificateV1 {
        ori_collision::certify_layered_four_face_chain_continuous_path_v1(
            self.same_semantics_other_requested
                .initial()
                .target()
                .model(),
            self.same_semantics_other_requested.initial().pose(),
            &self.same_semantics_other_target_angles(),
            &self.same_semantics_other_admission,
            self.limits,
        )
        .expect("reissue the semantically equal independent four-face certificate")
    }

    fn alternate_admission(
        &self,
    ) -> NativeStackedFoldInitialSampleLayerAdmissionV1<StackedFoldInitialLayerOrderV1> {
        ori_collision::prepare_stacked_fold_initial_sample_layer_admission_v1(
            self.requested.initial().target().model(),
            self.requested.initial().pose(),
            0.0,
            self.limits.static_limits,
            &self.initial_layer_order,
        )
        .expect("independently issued four-face initial admission")
    }

    fn applied_pose(&self, moving_target_ulp_delta: u64) -> AppliedPoseV1 {
        layered_applied_pose_v1(&self.requested, moving_target_ulp_delta)
    }

    fn ticket_with_anchor(
        &self,
        editor_instance_anchor: Arc<()>,
    ) -> SpeculativeUnprovenFoldResolutionTicketV1 {
        layered_ticket_with_anchor_v1(&self.requested, editor_instance_anchor)
    }

    fn ticket(&self) -> SpeculativeUnprovenFoldResolutionTicketV1 {
        self.ticket_with_anchor(Arc::new(()))
    }
}

fn fixed_four_face_id_v1<T: serde::de::DeserializeOwned>(
    prefix: &str,
    namespace: u64,
    index: u64,
) -> T {
    serde_json::from_str(&format!(
        "\"00000000-0000-4000-{prefix}-{:012x}\"",
        namespace * 100 + index
    ))
    .expect("fixed four-face test id")
}

fn three_face_points_v1() -> [(f64, f64); 8] {
    [
        (0.0, 0.0),
        (50.0, 0.0),
        (250.0, 0.0),
        (300.0, 0.0),
        (300.0, 100.0),
        (250.0, 100.0),
        (50.0, 100.0),
        (0.0, 100.0),
    ]
}

fn three_face_crease_v1(index: usize) -> ExpectedStackedFoldCreaseV1 {
    let points = three_face_points_v1();
    let (start, end, kind) = match index {
        0 => (points[1], points[6], EdgeKind::Mountain),
        1 => (points[2], points[5], EdgeKind::Valley),
        _ => panic!("three-face crease index out of range"),
    };
    ExpectedStackedFoldCreaseV1 {
        start: Point2::new(start.0, start.1),
        end: Point2::new(end.0, end.1),
        kind,
    }
}

fn three_face_strip_source_v1(
    namespace: u64,
    moving_crease: usize,
) -> (ProjectId, CreasePattern, Paper) {
    assert!(moving_crease < 2, "the three-face strip has two hinges");
    let points = three_face_points_v1();
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_four_face_id_v1("c510", namespace, index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_four_face_id_v1("c511", namespace, index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let stationary_crease = 1 - moving_crease;
    let (start, end, kind) = match stationary_crease {
        0 => (1, 6, EdgeKind::Mountain),
        1 => (2, 5, EdgeKind::Valley),
        _ => unreachable!(),
    };
    edges.push(Edge {
        id: fixed_four_face_id_v1("c511", namespace, stationary_crease as u64 + 11),
        start: boundary[start],
        end: boundary[end],
        kind,
    });
    let mut paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    paper.thickness_mm = 0.0;
    (
        fixed_four_face_id_v1("c512", namespace, 1),
        CreasePattern { vertices, edges },
        paper,
    )
}

fn four_face_points_v1() -> [(f64, f64); 10] {
    [
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
    ]
}

fn four_face_crease_v1(index: usize) -> ExpectedStackedFoldCreaseV1 {
    let points = four_face_points_v1();
    let (start, end, kind) = match index {
        0 => (points[1], points[8], EdgeKind::Mountain),
        1 => (points[2], points[7], EdgeKind::Valley),
        2 => (points[3], points[6], EdgeKind::Mountain),
        _ => panic!("four-face crease index out of range"),
    };
    ExpectedStackedFoldCreaseV1 {
        start: Point2::new(start.0, start.1),
        end: Point2::new(end.0, end.1),
        kind,
    }
}

fn layered_strip_source_v1(
    namespace: u64,
    crease_indexes: &[usize],
    moving_crease: usize,
) -> (ProjectId, CreasePattern, Paper) {
    assert!(
        crease_indexes.contains(&moving_crease),
        "the moving crease belongs to the layered strip"
    );
    assert!(
        crease_indexes.iter().all(|index| *index < 3),
        "the layered strip only defines three crease candidates"
    );
    let points = four_face_points_v1();
    let vertices = points
        .iter()
        .enumerate()
        .map(|(index, &(x, y))| Vertex {
            id: fixed_four_face_id_v1("c520", namespace, index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_four_face_id_v1("c521", namespace, index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    for &index in crease_indexes {
        if index == moving_crease {
            continue;
        }
        let (start, end, kind) = match index {
            0 => (1, 8, EdgeKind::Mountain),
            1 => (2, 7, EdgeKind::Valley),
            2 => (3, 6, EdgeKind::Mountain),
            _ => unreachable!(),
        };
        edges.push(Edge {
            id: fixed_four_face_id_v1("c521", namespace, index as u64 + 11),
            start: boundary[start],
            end: boundary[end],
            kind,
        });
    }
    let mut paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    paper.thickness_mm = 0.0;
    (
        fixed_four_face_id_v1("c522", namespace, 1),
        CreasePattern { vertices, edges },
        paper,
    )
}

struct LayeredBinderPremiseV1 {
    requested: PreparedStackedFoldRequestedPoseV1,
    same_semantics_other_requested: PreparedStackedFoldRequestedPoseV1,
    initial_layer_order: StackedFoldInitialLayerOrderV1,
    admission: NativeStackedFoldInitialSampleLayerAdmissionV1<StackedFoldInitialLayerOrderV1>,
    same_semantics_other_admission:
        NativeStackedFoldInitialSampleLayerAdmissionV1<StackedFoldInitialLayerOrderV1>,
}

fn prepare_layered_binder_premises_v1(
    namespace: u64,
    crease_indexes: &[usize],
    moving_crease: usize,
) -> Vec<LayeredBinderPremiseV1> {
    let (identity, source_pattern, source_paper) =
        layered_strip_source_v1(namespace, crease_indexes, moving_crease);
    prepare_layered_binder_premises_from_source_v1(
        identity,
        source_pattern,
        source_paper,
        four_face_crease_v1(moving_crease),
        crease_indexes.len() + 1,
        crease_indexes.len(),
        90.0,
    )
}

fn prepare_three_face_binder_premises_v1(
    namespace: u64,
    moving_crease: usize,
) -> Vec<LayeredBinderPremiseV1> {
    let (identity, source_pattern, source_paper) =
        three_face_strip_source_v1(namespace, moving_crease);
    prepare_layered_binder_premises_from_source_v1(
        identity,
        source_pattern,
        source_paper,
        three_face_crease_v1(moving_crease),
        3,
        2,
        45.0,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_layered_binder_premises_from_source_v1(
    identity: ProjectId,
    source_pattern: CreasePattern,
    source_paper: Paper,
    moving_crease: ExpectedStackedFoldCreaseV1,
    target_face_count: usize,
    target_hinge_count: usize,
    requested_angle_degrees: f64,
) -> Vec<LayeredBinderPremiseV1> {
    let source_revision = 1;
    let Some(source_topology) = analyze_faces(FaceExtractionInput {
        identity_namespace: identity,
        source_revision,
        paper: &source_paper,
        pattern: &source_pattern,
    })
    .snapshot
    else {
        return Vec::new();
    };
    let local = analyze_local_flat_foldability(&source_paper, &source_pattern);
    let Some(source_layer_order) = analyze_global_flat_foldability(
        GlobalFlatFoldabilityInput::current_with_geometry(
            identity,
            &source_paper,
            &source_pattern,
            &source_topology,
            &local,
        ),
        GlobalFlatFoldabilityLimits::default(),
    )
    .ok()
    .and_then(|proof| proof.layer_order().cloned()) else {
        return Vec::new();
    };
    let Ok(source_model) = MaterialTreeKinematicsModel::prepare(
        &source_pattern,
        &source_paper,
        &source_topology,
        TreeKinematicsLimits::default(),
    ) else {
        return Vec::new();
    };
    let Ok(source_angles) = CanonicalHingeAngles::new(
        source_model
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 180.0))
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_default(),
    ) else {
        return Vec::new();
    };
    if source_angles.as_slice().len() + 1 != target_hinge_count {
        return Vec::new();
    }

    let max_layer_pairs = target_face_count
        .checked_mul(target_face_count.saturating_sub(1))
        .and_then(|pairs| pairs.checked_div(2))
        .expect("small layered strip pair count");
    let mut premises = Vec::new();
    for fixed_face in source_model.face_ids() {
        let Ok(source_pose) = source_model.solve(Some(*fixed_face), &source_angles) else {
            continue;
        };
        let Ok(geometry) = prepare_stacked_fold_geometry_candidate_v1(
            identity,
            source_revision,
            &source_pattern,
            &source_paper,
            &source_layer_order,
            std::slice::from_ref(&moving_crease),
            StackedFoldTopologyBuildLimitsV1::default(),
            FaceLineageLimits::default(),
            StackedFoldGeometryLimitsV1::default(),
        ) else {
            continue;
        };
        let same_semantics_other_geometry = geometry.clone();
        let Ok(target) =
            prepare_stacked_fold_target_model_v1(geometry, TreeKinematicsLimits::default())
        else {
            continue;
        };
        if target.model().face_ids().len() != target_face_count
            || target.model().hinges().len() != target_hinge_count
        {
            continue;
        }
        let Ok(same_semantics_other_target) = prepare_stacked_fold_target_model_v1(
            same_semantics_other_geometry,
            TreeKinematicsLimits::default(),
        ) else {
            continue;
        };
        let Ok(initial) = prepare_stacked_fold_initial_pose_v1(target, &source_model, &source_pose)
        else {
            continue;
        };
        let Ok(same_semantics_other_initial) = prepare_stacked_fold_initial_pose_v1(
            same_semantics_other_target,
            &source_model,
            &source_pose,
        ) else {
            continue;
        };
        let Ok(initial_layer_order) =
            crate::stacked_fold::prepare_stacked_fold_initial_layer_order_v1(
                &initial,
                &source_layer_order,
                max_layer_pairs,
            )
        else {
            continue;
        };
        let Ok(same_semantics_other_initial_layer_order) =
            crate::stacked_fold::prepare_stacked_fold_initial_layer_order_v1(
                &same_semantics_other_initial,
                &source_layer_order,
                max_layer_pairs,
            )
        else {
            continue;
        };
        let Ok(requested) =
            prepare_stacked_fold_requested_pose_v1(initial, requested_angle_degrees)
        else {
            continue;
        };
        let Ok(same_semantics_other_requested) = prepare_stacked_fold_requested_pose_v1(
            same_semantics_other_initial,
            requested_angle_degrees,
        ) else {
            continue;
        };
        let Ok(admission) = ori_collision::prepare_stacked_fold_initial_sample_layer_admission_v1(
            requested.initial().target().model(),
            requested.initial().pose(),
            0.0,
            ori_collision::StaticCollisionLimits::default(),
            &initial_layer_order,
        ) else {
            continue;
        };
        let Ok(same_semantics_other_admission) =
            ori_collision::prepare_stacked_fold_initial_sample_layer_admission_v1(
                same_semantics_other_requested.initial().target().model(),
                same_semantics_other_requested.initial().pose(),
                0.0,
                ori_collision::StaticCollisionLimits::default(),
                &same_semantics_other_initial_layer_order,
            )
        else {
            continue;
        };
        premises.push(LayeredBinderPremiseV1 {
            requested,
            same_semantics_other_requested,
            initial_layer_order,
            admission,
            same_semantics_other_admission,
        });
    }
    premises
}

fn maximum_layered_dyadic_depth_v1() -> u8 {
    let maximum_leaves = ori_collision::MAX_DYADIC_FACE_TRANSFORM_LEAVES_V1;
    assert!(maximum_leaves.is_power_of_two());
    u8::try_from(maximum_leaves.ilog2()).expect("the global dyadic cap has a bounded depth")
}

fn build_three_face_binder_fixture_v1(namespace: u64) -> ThreeFaceBinderFixtureV1 {
    for moving_crease in 0..2 {
        for premise in prepare_three_face_binder_premises_v1(namespace, moving_crease) {
            let target_angles = layered_target_angles_v1(&premise.requested);
            for dyadic_depth in 0_u8..=maximum_layered_dyadic_depth_v1() {
                let limits = LayeredThreeFaceContinuousLimitsV1 {
                    dyadic_depth,
                    max_leaves: 1_usize << dyadic_depth,
                    ..LayeredThreeFaceContinuousLimitsV1::default()
                };
                if ori_collision::certify_layered_three_face_continuous_path_v1(
                    premise.requested.initial().target().model(),
                    premise.requested.initial().pose(),
                    &target_angles,
                    &premise.admission,
                    limits,
                )
                .is_ok()
                {
                    return ThreeFaceBinderFixtureV1 {
                        requested: premise.requested,
                        same_semantics_other_requested: premise.same_semantics_other_requested,
                        initial_layer_order: premise.initial_layer_order,
                        admission: premise.admission,
                        same_semantics_other_admission: premise.same_semantics_other_admission,
                        limits,
                    };
                }
            }
        }
    }
    panic!("no production-valid core three-face binder fixture");
}

fn build_four_face_binder_fixture_v1(namespace: u64) -> FourFaceBinderFixtureV1 {
    let crease_indexes = [0, 1, 2];
    for moving_crease in crease_indexes {
        for premise in prepare_layered_binder_premises_v1(namespace, &crease_indexes, moving_crease)
        {
            let target_angles = layered_target_angles_v1(&premise.requested);
            for dyadic_depth in 0_u8..=maximum_layered_dyadic_depth_v1() {
                let limits = LayeredFourFaceChainContinuousLimitsV1 {
                    dyadic_depth,
                    max_leaves: 1_usize << dyadic_depth,
                    ..LayeredFourFaceChainContinuousLimitsV1::default()
                };
                if ori_collision::certify_layered_four_face_chain_continuous_path_v1(
                    premise.requested.initial().target().model(),
                    premise.requested.initial().pose(),
                    &target_angles,
                    &premise.admission,
                    limits,
                )
                .is_ok()
                {
                    return FourFaceBinderFixtureV1 {
                        requested: premise.requested,
                        same_semantics_other_requested: premise.same_semantics_other_requested,
                        initial_layer_order: premise.initial_layer_order,
                        admission: premise.admission,
                        same_semantics_other_admission: premise.same_semantics_other_admission,
                        limits,
                    };
                }
            }
        }
    }
    panic!("no production-valid core four-face binder fixture");
}

fn assert_four_face_recoverable_failure_v1(
    result: Result<
        SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1,
        SpeculativeUnprovenFoldLayeredFourFaceCertificationFailureV1,
    >,
    expected_error: SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1,
    expected_ticket: &ResolutionTicketSnapshotV1,
) -> (
    SpeculativeUnprovenFoldResolutionTicketV1,
    LayeredFourFaceChainContinuousCertificateV1,
) {
    let failure = result.expect_err("four-face binder must fail closed");
    assert_eq!(failure.error(), &expected_error);
    let (error, ticket, certificate) = failure.into_parts();
    assert_eq!(error, expected_error);
    assert_eq!(
        ResolutionTicketSnapshotV1::capture(&ticket),
        *expected_ticket
    );
    (ticket, certificate)
}

fn assert_three_face_recoverable_failure_v1(
    result: Result<
        SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1,
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationFailureV1,
    >,
    expected_error: SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1,
    expected_ticket: &ResolutionTicketSnapshotV1,
) -> (
    SpeculativeUnprovenFoldResolutionTicketV1,
    LayeredThreeFaceContinuousCertificateV1,
) {
    let failure = result.expect_err("three-face binder must fail closed");
    assert_eq!(failure.error(), &expected_error);
    let (error, ticket, certificate) = failure.into_parts();
    assert_eq!(error, expected_error);
    assert_eq!(
        ResolutionTicketSnapshotV1::capture(&ticket),
        *expected_ticket
    );
    (ticket, certificate)
}

fn retry_exact_three_face_authority_v1(
    fixture: &ThreeFaceBinderFixtureV1,
    ticket: SpeculativeUnprovenFoldResolutionTicketV1,
    certificate: LayeredThreeFaceContinuousCertificateV1,
) {
    bind_speculative_unproven_layered_three_face_continuous_proof_v1(
        ticket,
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        certificate,
    )
    .expect("the recovered three-face ticket and certificate remain exactly retryable");
}

#[test]
fn three_face_ticket_rejects_same_semantics_prepared_request_aba_and_exactly_retries() {
    let fixture = build_three_face_binder_fixture_v1(50);
    let owner_model = fixture.requested.initial().target().model();
    let other_model = fixture
        .same_semantics_other_requested
        .initial()
        .target()
        .model();
    assert_eq!(
        fixture.requested.initial().target().geometry(),
        fixture
            .same_semantics_other_requested
            .initial()
            .target()
            .geometry()
    );
    assert_eq!(
        fixture.requested.pose().hinge_angles(),
        fixture.same_semantics_other_requested.pose().hinge_angles()
    );
    assert!(
        !owner_model.owns_pose(fixture.same_semantics_other_requested.initial().pose())
            && !owner_model.owns_pose(fixture.same_semantics_other_requested.pose())
            && !other_model.owns_pose(fixture.requested.initial().pose())
            && !other_model.owns_pose(fixture.requested.pose()),
        "the three-face ABA request must use a separately prepared model and poses"
    );

    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let (ticket, other_certificate) = assert_three_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_three_face_continuous_proof_v1(
            ticket,
            &fixture.same_semantics_other_requested,
            &fixture.same_semantics_other_admission,
            fixture.limits,
            fixture.issue_same_semantics_other_certificate(),
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Common(
            SpeculativeUnprovenFoldCertificationErrorV1::PreparedRequestIssuerMismatch,
        ),
        &snapshot,
    );
    assert!(
        other_certificate.is_for(
            other_model,
            fixture.same_semantics_other_requested.initial().pose(),
            &fixture.same_semantics_other_target_angles(),
            &fixture.same_semantics_other_admission,
            fixture.limits,
        ),
        "the rejected independent three-face certificate must be returned unchanged"
    );
    retry_exact_three_face_authority_v1(&fixture, ticket, fixture.issue_certificate());
}

#[test]
fn three_face_binder_production_fixture_covers_read_only_and_exact_identity_boundaries() {
    let fixture = build_three_face_binder_fixture_v1(43);

    let ticket = fixture.ticket();
    let expected_binding = ticket.binding.clone();
    let expected_revision = ticket.target_revision;
    let expected_fingerprint = ticket.target_geometry_fingerprint;
    let expected_pose = ticket.target_applied_pose.clone();
    let proof = bind_speculative_unproven_layered_three_face_continuous_proof_v1(
        ticket,
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        fixture.issue_certificate(),
    )
    .expect("exact production three-face authority binds");
    assert!(!proof.authorizes_project_mutation());
    assert_eq!(proof.binding(), &expected_binding);
    assert_eq!(proof.target_revision(), expected_revision);
    assert_eq!(proof.target_geometry_fingerprint(), &expected_fingerprint);
    assert_eq!(proof.target_applied_pose(), &expected_pose);
    assert!(proof.has_same_editor_instance_as(&proof));

    let mut one_ulp_ticket = fixture.ticket();
    one_ulp_ticket.target_applied_pose = fixture.applied_pose(1);
    let snapshot = ResolutionTicketSnapshotV1::capture(&one_ulp_ticket);
    let (mut one_ulp_ticket, certificate) = assert_three_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_three_face_continuous_proof_v1(
            one_ulp_ticket,
            &fixture.requested,
            &fixture.admission,
            fixture.limits,
            fixture.issue_certificate(),
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Common(
            SpeculativeUnprovenFoldCertificationErrorV1::TargetAppliedPoseMismatch,
        ),
        &snapshot,
    );
    one_ulp_ticket.target_applied_pose = fixture.applied_pose(0);
    retry_exact_three_face_authority_v1(&fixture, one_ulp_ticket, certificate);

    let alternate_admission = fixture.alternate_admission();
    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let (ticket, certificate) = assert_three_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_three_face_continuous_proof_v1(
            ticket,
            &fixture.requested,
            &alternate_admission,
            fixture.limits,
            fixture.issue_certificate(),
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::LayeredCertificateMismatch,
        &snapshot,
    );
    retry_exact_three_face_authority_v1(&fixture, ticket, certificate);

    let drifted_limits = LayeredThreeFaceContinuousLimitsV1 {
        max_leaves: fixture
            .limits
            .max_leaves
            .checked_add(1)
            .expect("bounded three-face test leaf limit"),
        ..fixture.limits
    };
    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let (ticket, certificate) = assert_three_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_three_face_continuous_proof_v1(
            ticket,
            &fixture.requested,
            &fixture.admission,
            drifted_limits,
            fixture.issue_certificate(),
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::LayeredCertificateMismatch,
        &snapshot,
    );
    retry_exact_three_face_authority_v1(&fixture, ticket, certificate);

    let source_model = fixture.requested.initial().target().model();
    let alternate_fixed_face = source_model
        .face_ids()
        .iter()
        .copied()
        .find(|face| Some(*face) != fixture.requested.initial().pose().fixed_face())
        .expect("alternate three-face fixed face");
    let alternate_source_pose = source_model
        .solve(
            Some(alternate_fixed_face),
            &CanonicalHingeAngles::new(fixture.requested.initial().pose().hinge_angles().to_vec())
                .expect("canonical three-face source angles"),
        )
        .expect("independent three-face source-pose instance");
    let source_bound_certificate = fixture.issue_certificate();
    assert!(
        !source_bound_certificate.is_for(
            source_model,
            &alternate_source_pose,
            &fixture.target_angles(),
            &fixture.admission,
            fixture.limits,
        ),
        "source-pose identity is part of three-face certificate authority"
    );
    retry_exact_three_face_authority_v1(&fixture, fixture.ticket(), source_bound_certificate);

    let foreign_fixture = build_three_face_binder_fixture_v1(44);
    let model_bound_certificate = fixture.issue_certificate();
    assert!(
        !model_bound_certificate.is_for(
            foreign_fixture.requested.initial().target().model(),
            foreign_fixture.requested.initial().pose(),
            &foreign_fixture.target_angles(),
            &foreign_fixture.admission,
            foreign_fixture.limits,
        ),
        "foreign model, source pose, and issuer cannot impersonate the certificate"
    );
    retry_exact_three_face_authority_v1(&fixture, fixture.ticket(), model_bound_certificate);

    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let (_ticket, foreign_certificate) = assert_three_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_three_face_continuous_proof_v1(
            ticket,
            &fixture.requested,
            &fixture.admission,
            fixture.limits,
            foreign_fixture.issue_certificate(),
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::LayeredCertificateMismatch,
        &snapshot,
    );
    retry_exact_three_face_authority_v1(
        &foreign_fixture,
        foreign_fixture.ticket(),
        foreign_certificate,
    );
}

#[test]
fn three_face_binder_production_fixture_recovers_stops_faults_and_late_ownership() {
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };

    let fixture = build_three_face_binder_fixture_v1(45);

    let cancelled = AtomicBool::new(true);
    let cancelled_control = CooperativeOperationControlV1::new(
        Some(&cancelled),
        Instant::now() + Duration::from_secs(1),
    );
    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let (ticket, certificate) = assert_three_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_three_face_continuous_proof_with_control_v1(
            ticket,
            &fixture.requested,
            &fixture.admission,
            fixture.limits,
            fixture.issue_certificate(),
            &cancelled_control,
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Cancelled,
        &snapshot,
    );
    retry_exact_three_face_authority_v1(&fixture, ticket, certificate);

    let expired_control = CooperativeOperationControlV1::new(None, Instant::now());
    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let (ticket, certificate) = assert_three_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_three_face_continuous_proof_with_control_v1(
            ticket,
            &fixture.requested,
            &fixture.admission,
            fixture.limits,
            fixture.issue_certificate(),
            &expired_control,
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::DeadlineExceeded,
        &snapshot,
    );
    retry_exact_three_face_authority_v1(&fixture, ticket, certificate);

    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let allocation_result = {
        let _fault =
            CertificationTestFaultGuardV1::set(CertificationTestFaultV1::TargetAngleAllocation);
        bind_speculative_unproven_layered_three_face_continuous_proof_v1(
            ticket,
            &fixture.requested,
            &fixture.admission,
            fixture.limits,
            fixture.issue_certificate(),
        )
    };
    let (ticket, certificate) = assert_three_face_recoverable_failure_v1(
        allocation_result,
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Common(
            SpeculativeUnprovenFoldCertificationErrorV1::RequestedTargetAngleAllocationFailed,
        ),
        &snapshot,
    );
    retry_exact_three_face_authority_v1(&fixture, ticket, certificate);

    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let panic_result = {
        let _fault =
            CertificationTestFaultGuardV1::set(CertificationTestFaultV1::NativeRevalidationPanic);
        bind_speculative_unproven_layered_three_face_continuous_proof_v1(
            ticket,
            &fixture.requested,
            &fixture.admission,
            fixture.limits,
            fixture.issue_certificate(),
        )
    };
    let (ticket, certificate) = assert_three_face_recoverable_failure_v1(
        panic_result,
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Common(
            SpeculativeUnprovenFoldCertificationErrorV1::ValidationPanicked,
        ),
        &snapshot,
    );
    retry_exact_three_face_authority_v1(&fixture, ticket, certificate);

    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let late_stop_result = {
        let _fault =
            CertificationTestFaultGuardV1::set(CertificationTestFaultV1::LateOwnershipCancellation);
        bind_speculative_unproven_layered_three_face_continuous_proof_with_control_v1(
            ticket,
            &fixture.requested,
            &fixture.admission,
            fixture.limits,
            fixture.issue_certificate(),
            &CooperativeOperationControlV1::unbounded(),
        )
    };
    let (ticket, certificate) = assert_three_face_recoverable_failure_v1(
        late_stop_result,
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Cancelled,
        &snapshot,
    );
    retry_exact_three_face_authority_v1(&fixture, ticket, certificate);
}

#[test]
fn four_face_ticket_rejects_same_semantics_prepared_request_aba_and_exactly_retries() {
    let fixture = build_four_face_binder_fixture_v1(51);
    let owner_model = fixture.requested.initial().target().model();
    let other_model = fixture
        .same_semantics_other_requested
        .initial()
        .target()
        .model();
    assert_eq!(
        fixture.requested.initial().target().geometry(),
        fixture
            .same_semantics_other_requested
            .initial()
            .target()
            .geometry()
    );
    assert_eq!(
        fixture.requested.pose().hinge_angles(),
        fixture.same_semantics_other_requested.pose().hinge_angles()
    );
    assert!(
        !owner_model.owns_pose(fixture.same_semantics_other_requested.initial().pose())
            && !owner_model.owns_pose(fixture.same_semantics_other_requested.pose())
            && !other_model.owns_pose(fixture.requested.initial().pose())
            && !other_model.owns_pose(fixture.requested.pose()),
        "the four-face ABA request must use a separately prepared model and poses"
    );

    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let (ticket, other_certificate) = assert_four_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
            ticket,
            &fixture.same_semantics_other_requested,
            &fixture.same_semantics_other_admission,
            fixture.limits,
            fixture.issue_same_semantics_other_certificate(),
        ),
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::Common(
            SpeculativeUnprovenFoldCertificationErrorV1::PreparedRequestIssuerMismatch,
        ),
        &snapshot,
    );
    assert!(
        other_certificate.is_for(
            other_model,
            fixture.same_semantics_other_requested.initial().pose(),
            &fixture.same_semantics_other_target_angles(),
            &fixture.same_semantics_other_admission,
            fixture.limits,
        ),
        "the rejected independent four-face certificate must be returned unchanged"
    );
    bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
        ticket,
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        fixture.issue_certificate(),
    )
    .expect("the recovered four-face owner ticket remains exactly retryable");
}

#[test]
fn layered_entry_checkpoint_precedes_metadata_and_resource_validation() {
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };

    let three_face = build_three_face_binder_fixture_v1(47);
    let foreign_three_face = build_three_face_binder_fixture_v1(48);
    let cancelled = AtomicBool::new(true);
    let cancelled_control = CooperativeOperationControlV1::new(
        Some(&cancelled),
        Instant::now() + Duration::from_secs(1),
    );
    let ticket = three_face.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let (ticket, certificate) = assert_three_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_three_face_continuous_proof_with_control_v1(
            ticket,
            &foreign_three_face.requested,
            &foreign_three_face.admission,
            foreign_three_face.limits,
            three_face.issue_certificate(),
            &cancelled_control,
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Cancelled,
        &snapshot,
    );
    retry_exact_three_face_authority_v1(&three_face, ticket, certificate);

    let four_face = build_four_face_binder_fixture_v1(49);
    let expired_control = CooperativeOperationControlV1::new(None, Instant::now());
    let ticket = four_face.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let result = {
        let _fault =
            CertificationTestFaultGuardV1::set(CertificationTestFaultV1::TargetAngleAllocation);
        bind_speculative_unproven_layered_four_face_chain_continuous_proof_with_control_v1(
            ticket,
            &four_face.requested,
            &four_face.admission,
            four_face.limits,
            four_face.issue_certificate(),
            &expired_control,
        )
    };
    let (ticket, certificate) = assert_four_face_recoverable_failure_v1(
        result,
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::DeadlineExceeded,
        &snapshot,
    );
    bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
        ticket,
        &four_face.requested,
        &four_face.admission,
        four_face.limits,
        certificate,
    )
    .expect("the pre-validation deadline returns the exact four-face retry authority");
}

#[test]
fn three_face_binder_production_fixture_rejects_foreign_resolver_and_maps_resources() {
    let fixture = build_three_face_binder_fixture_v1(46);
    let owner_editor = crate::EditorState::new(CreasePattern::empty());
    let mut foreign_editor = crate::EditorState::new(CreasePattern::empty());
    let proof = bind_speculative_unproven_layered_three_face_continuous_proof_v1(
        fixture.ticket_with_anchor(Arc::clone(&owner_editor.runtime_instance_anchor)),
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        fixture.issue_certificate(),
    )
    .expect("owner-anchored production three-face proof");
    assert!(!proof.authorizes_project_mutation());
    assert!(matches!(
        foreign_editor.resolve_speculative_unproven_fold_layered_three_face_certified_v1(proof),
        Err(crate::SpeculativeUnprovenFoldResolutionErrorV1::ForeignEditor)
    ));

    assert_eq!(
        map_layered_certificate_revalidation_error_v1(LayeredThreeFaceContinuousErrorV1::Cancelled),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Cancelled
    );
    assert_eq!(
        map_layered_certificate_revalidation_error_v1(
            LayeredThreeFaceContinuousErrorV1::DeadlineExceeded
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::DeadlineExceeded
    );
    assert_eq!(
        map_layered_certificate_revalidation_error_v1(
            LayeredThreeFaceContinuousErrorV1::ResourceLimit
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::ResourceUnavailable
    );
    assert_eq!(
        map_layered_certificate_revalidation_error_v1(
            LayeredThreeFaceContinuousErrorV1::IssuerMismatch
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::LayeredCertificateMismatch
    );
}

#[test]
fn layered_binder_stop_and_fault_seams_preserve_the_distinct_error_surface() {
    let fixture = certification_fixture(37.0);
    let cancelled = std::sync::atomic::AtomicBool::new(true);
    let cancelled_control = CooperativeOperationControlV1::new(
        Some(&cancelled),
        std::time::Instant::now() + std::time::Duration::from_secs(1),
    );
    assert_eq!(
        validate_layered_ticket_request_v1(&fixture.ticket, &fixture.requested, &cancelled_control)
            .expect_err("a pre-cancelled layered binder must stop before native authority"),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Cancelled
    );

    let expired_control = CooperativeOperationControlV1::new(None, std::time::Instant::now());
    assert_eq!(
        validate_layered_ticket_request_v1(&fixture.ticket, &fixture.requested, &expired_control)
            .expect_err("an expired layered binder must stop before native authority"),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::DeadlineExceeded
    );

    let allocation = {
        let _fault =
            CertificationTestFaultGuardV1::set(CertificationTestFaultV1::TargetAngleAllocation);
        validate_layered_ticket_request_v1(
            &fixture.ticket,
            &fixture.requested,
            &CooperativeOperationControlV1::unbounded(),
        )
        .expect_err("the layered binder must use the recoverable angle allocation seam")
    };
    assert_eq!(
        allocation,
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Common(
            SpeculativeUnprovenFoldCertificationErrorV1::RequestedTargetAngleAllocationFailed
        )
    );
    assert_eq!(
        map_layered_certificate_revalidation_error_v1(LayeredThreeFaceContinuousErrorV1::Cancelled),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::Cancelled
    );
    assert_eq!(
        map_layered_certificate_revalidation_error_v1(
            LayeredThreeFaceContinuousErrorV1::DeadlineExceeded
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::DeadlineExceeded
    );
    assert_eq!(
        map_layered_certificate_revalidation_error_v1(
            LayeredThreeFaceContinuousErrorV1::ResourceLimit
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::ResourceUnavailable
    );
    assert_eq!(
        map_layered_certificate_revalidation_error_v1(
            LayeredThreeFaceContinuousErrorV1::InitialLayerAdmissionUnavailable
        ),
        SpeculativeUnprovenFoldLayeredThreeFaceCertificationErrorV1::LayeredCertificateMismatch
    );
}

#[test]
fn layered_proof_exposes_only_read_only_provenance() {
    let CertificationFixture { ticket, .. } = certification_fixture(37.0);
    let expected_binding = ticket.binding.clone();
    let expected_revision = ticket.target_revision;
    let expected_fingerprint = ticket.target_geometry_fingerprint;
    let expected_pose = ticket.target_applied_pose.clone();
    let SpeculativeUnprovenFoldResolutionTicketV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
        prepared_request_issuer_seal: _,
    } = ticket;
    let proof = SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1 {
        editor_instance_anchor,
        binding,
        target_revision,
        target_geometry_fingerprint,
        target_applied_pose,
    };

    assert!(!proof.authorizes_project_mutation());
    assert_eq!(proof.binding(), &expected_binding);
    assert_eq!(proof.target_revision(), expected_revision);
    assert_eq!(proof.target_geometry_fingerprint(), &expected_fingerprint);
    assert_eq!(proof.target_applied_pose(), &expected_pose);
    assert!(proof.has_same_editor_instance_as(&proof));
}

#[test]
fn four_face_binder_production_fixture_covers_exact_recovery_and_type_boundaries() {
    use std::{
        any::TypeId,
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };

    let fixture = build_four_face_binder_fixture_v1(41);

    assert_ne!(
        TypeId::of::<SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1>(),
        TypeId::of::<SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1>()
    );
    assert_ne!(
        TypeId::of::<SpeculativeUnprovenFoldLayeredFourFaceCertifiedProofV1>(),
        TypeId::of::<SpeculativeUnprovenFoldCertifiedProofV1>()
    );
    assert_ne!(
        TypeId::of::<SpeculativeUnprovenFoldLayeredThreeFaceCertifiedProofV1>(),
        TypeId::of::<SpeculativeUnprovenFoldCertifiedProofV1>()
    );

    let ticket = fixture.ticket();
    let expected_binding = ticket.binding.clone();
    let expected_revision = ticket.target_revision;
    let expected_fingerprint = ticket.target_geometry_fingerprint;
    let expected_pose = ticket.target_applied_pose.clone();
    let proof = bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
        ticket,
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        fixture.issue_certificate(),
    )
    .expect("exact four-face ticket/request/admission/limits/certificate bind");
    assert!(!proof.authorizes_project_mutation());
    assert_eq!(proof.binding(), &expected_binding);
    assert_eq!(proof.target_revision(), expected_revision);
    assert_eq!(proof.target_geometry_fingerprint(), &expected_fingerprint);
    assert_eq!(proof.target_applied_pose(), &expected_pose);
    assert!(proof.has_same_editor_instance_as(&proof));

    let mut one_ulp_ticket = fixture.ticket();
    one_ulp_ticket.target_applied_pose = fixture.applied_pose(1);
    let one_ulp_snapshot = ResolutionTicketSnapshotV1::capture(&one_ulp_ticket);
    let (mut one_ulp_ticket, certificate) = assert_four_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
            one_ulp_ticket,
            &fixture.requested,
            &fixture.admission,
            fixture.limits,
            fixture.issue_certificate(),
        ),
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::Common(
            SpeculativeUnprovenFoldCertificationErrorV1::TargetAppliedPoseMismatch,
        ),
        &one_ulp_snapshot,
    );
    one_ulp_ticket.target_applied_pose = fixture.applied_pose(0);
    bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
        one_ulp_ticket,
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        certificate,
    )
    .expect("the recovered certificate binds after exact semantic-pose repair");

    let alternate_admission = fixture.alternate_admission();
    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let (ticket, certificate) = assert_four_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
            ticket,
            &fixture.requested,
            &alternate_admission,
            fixture.limits,
            fixture.issue_certificate(),
        ),
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::LayeredCertificateMismatch,
        &snapshot,
    );
    bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
        ticket,
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        certificate,
    )
    .expect("alternate-admission failure returns the exact retry authority");

    let drifted_limits = LayeredFourFaceChainContinuousLimitsV1 {
        max_leaves: fixture
            .limits
            .max_leaves
            .checked_add(1)
            .expect("bounded test leaf limit"),
        ..fixture.limits
    };
    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let (ticket, certificate) = assert_four_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
            ticket,
            &fixture.requested,
            &fixture.admission,
            drifted_limits,
            fixture.issue_certificate(),
        ),
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::LayeredCertificateMismatch,
        &snapshot,
    );
    bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
        ticket,
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        certificate,
    )
    .expect("limits failure returns the exact retry authority");

    let cancelled = AtomicBool::new(true);
    let cancelled_control = CooperativeOperationControlV1::new(
        Some(&cancelled),
        Instant::now() + Duration::from_secs(1),
    );
    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let (ticket, certificate) = assert_four_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_four_face_chain_continuous_proof_with_control_v1(
            ticket,
            &fixture.requested,
            &fixture.admission,
            fixture.limits,
            fixture.issue_certificate(),
            &cancelled_control,
        ),
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::Cancelled,
        &snapshot,
    );
    bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
        ticket,
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        certificate,
    )
    .expect("cancelled bind returns the exact retry authority");

    let expired_control = CooperativeOperationControlV1::new(None, Instant::now());
    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let (ticket, certificate) = assert_four_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_four_face_chain_continuous_proof_with_control_v1(
            ticket,
            &fixture.requested,
            &fixture.admission,
            fixture.limits,
            fixture.issue_certificate(),
            &expired_control,
        ),
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::DeadlineExceeded,
        &snapshot,
    );
    bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
        ticket,
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        certificate,
    )
    .expect("expired bind returns the exact retry authority");

    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let allocation_result = {
        let _fault =
            CertificationTestFaultGuardV1::set(CertificationTestFaultV1::TargetAngleAllocation);
        bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
            ticket,
            &fixture.requested,
            &fixture.admission,
            fixture.limits,
            fixture.issue_certificate(),
        )
    };
    let (ticket, certificate) = assert_four_face_recoverable_failure_v1(
        allocation_result,
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::Common(
            SpeculativeUnprovenFoldCertificationErrorV1::RequestedTargetAngleAllocationFailed,
        ),
        &snapshot,
    );
    bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
        ticket,
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        certificate,
    )
    .expect("allocation failure returns the exact retry authority");

    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let panic_result = {
        let _fault =
            CertificationTestFaultGuardV1::set(CertificationTestFaultV1::NativeRevalidationPanic);
        bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
            ticket,
            &fixture.requested,
            &fixture.admission,
            fixture.limits,
            fixture.issue_certificate(),
        )
    };
    let (ticket, certificate) = assert_four_face_recoverable_failure_v1(
        panic_result,
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::Common(
            SpeculativeUnprovenFoldCertificationErrorV1::ValidationPanicked,
        ),
        &snapshot,
    );
    bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
        ticket,
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        certificate,
    )
    .expect("caught panic returns the exact retry authority");

    let source_model = fixture.requested.initial().target().model();
    let alternate_fixed_face = source_model
        .face_ids()
        .iter()
        .copied()
        .find(|face| Some(*face) != fixture.requested.initial().pose().fixed_face())
        .expect("alternate four-face fixed face");
    let alternate_source_pose = source_model
        .solve(
            Some(alternate_fixed_face),
            &CanonicalHingeAngles::new(fixture.requested.initial().pose().hinge_angles().to_vec())
                .expect("canonical source angles"),
        )
        .expect("independent source pose instance");
    let source_bound_certificate = fixture.issue_certificate();
    assert!(
        !source_bound_certificate.is_for(
            source_model,
            &alternate_source_pose,
            &fixture.target_angles(),
            &fixture.admission,
            fixture.limits,
        ),
        "a distinct source-pose instance must not impersonate the certificate"
    );
    bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
        fixture.ticket(),
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        source_bound_certificate,
    )
    .expect("read-only source-pose rejection leaves the exact certificate valid");

    let foreign_fixture = build_four_face_binder_fixture_v1(42);
    let model_bound_certificate = fixture.issue_certificate();
    assert!(
        !model_bound_certificate.is_for(
            foreign_fixture.requested.initial().target().model(),
            foreign_fixture.requested.initial().pose(),
            &foreign_fixture.target_angles(),
            &foreign_fixture.admission,
            foreign_fixture.limits,
        ),
        "a separately prepared model/source/admission cannot impersonate the issuer"
    );
    bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
        fixture.ticket(),
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        model_bound_certificate,
    )
    .expect("foreign-model rejection leaves the exact certificate valid");

    let ticket = fixture.ticket();
    let snapshot = ResolutionTicketSnapshotV1::capture(&ticket);
    let _ = assert_four_face_recoverable_failure_v1(
        bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
            ticket,
            &fixture.requested,
            &fixture.admission,
            fixture.limits,
            foreign_fixture.issue_certificate(),
        ),
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::LayeredCertificateMismatch,
        &snapshot,
    );

    let owner_editor = crate::EditorState::new(CreasePattern::empty());
    let mut foreign_editor = crate::EditorState::new(CreasePattern::empty());
    let proof = bind_speculative_unproven_layered_four_face_chain_continuous_proof_v1(
        fixture.ticket_with_anchor(Arc::clone(&owner_editor.runtime_instance_anchor)),
        &fixture.requested,
        &fixture.admission,
        fixture.limits,
        fixture.issue_certificate(),
    )
    .expect("owner-anchored four-face proof");
    assert!(matches!(
        foreign_editor.resolve_speculative_unproven_fold_layered_four_face_certified_v1(proof),
        Err(crate::SpeculativeUnprovenFoldResolutionErrorV1::ForeignEditor)
    ));

    assert_eq!(
        map_layered_four_face_certificate_revalidation_error_v1(
            LayeredFourFaceChainContinuousErrorV1::Cancelled
        ),
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::Cancelled
    );
    assert_eq!(
        map_layered_four_face_certificate_revalidation_error_v1(
            LayeredFourFaceChainContinuousErrorV1::DeadlineExceeded
        ),
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::DeadlineExceeded
    );
    assert_eq!(
        map_layered_four_face_certificate_revalidation_error_v1(
            LayeredFourFaceChainContinuousErrorV1::ResourceLimit
        ),
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::ResourceUnavailable
    );
    assert_eq!(
        map_layered_four_face_certificate_revalidation_error_v1(
            LayeredFourFaceChainContinuousErrorV1::IssuerMismatch
        ),
        SpeculativeUnprovenFoldLayeredFourFaceCertificationErrorV1::LayeredCertificateMismatch
    );
}
