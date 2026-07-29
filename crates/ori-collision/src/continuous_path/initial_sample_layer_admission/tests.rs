use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId};
use ori_kinematics::{CanonicalHingeAngles, TreeKinematicsLimits};
use ori_topology::{FaceExtractionInput, analyze_faces};

use super::*;

struct ReadOnceSourceV1 {
    face: FaceId,
    transform: ExactAffineTransform,
    calls: Cell<u16>,
    cancel_after_observation: Option<(Arc<AtomicBool>, u16)>,
}

impl ReadOnceSourceV1 {
    fn observe(&self, bit: u16) {
        let calls = self.calls.get();
        assert_eq!(calls & bit, 0, "source observation was read twice");
        self.calls.set(calls | bit);
        if self
            .cancel_after_observation
            .as_ref()
            .is_some_and(|(_, trigger)| *trigger == bit)
        {
            self.cancel_after_observation
                .as_ref()
                .expect("checked above")
                .0
                .store(true, Ordering::Release);
        }
    }
}

impl NonFlatLayerOrderStructuralSourceV1 for ReadOnceSourceV1 {
    fn material_face_count(&self) -> usize {
        self.observe(1 << 0);
        1
    }

    fn material_face_id(&self, index: usize) -> Option<FaceId> {
        self.observe(1 << 1);
        (index == 0).then_some(self.face)
    }

    fn folded_face_count(&self) -> usize {
        self.observe(1 << 2);
        1
    }

    fn folded_face(&self, index: usize) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>> {
        self.observe(1 << 3);
        (index == 0).then_some(NonFlatFoldedFaceStructuralRefV1 {
            face_id: self.face,
            dropped_world_axis: 2,
            source_to_plane: &self.transform,
        })
    }

    fn overlap_cell_count(&self) -> usize {
        self.observe(1 << 4);
        0
    }

    fn overlap_cell(&self, _index: usize) -> Option<NonFlatOverlapCellStructuralRefV1<'_>> {
        panic!("zero declared cells must not be read")
    }

    fn face_pair_order_count(&self) -> usize {
        self.observe(1 << 5);
        0
    }

    fn face_pair_order(&self, _index: usize) -> Option<NonFlatFacePairOrderStructuralV1> {
        panic!("zero declared orders must not be read")
    }
}

impl StackedFoldInitialLayerOrderSourceV1 for ReadOnceSourceV1 {
    fn tested_face_pairs_v1(&self) -> usize {
        self.observe(1 << 6);
        0
    }

    fn fixed_face_v1(&self) -> Option<FaceId> {
        self.observe(1 << 7);
        Some(self.face)
    }

    fn hinge_angle_count_v1(&self) -> usize {
        self.observe(1 << 8);
        0
    }

    fn hinge_angle_v1(&self, _index: usize) -> Option<(EdgeId, u64)> {
        panic!("zero declared hinges must not be read")
    }

    fn paper_thickness_bits_v1(&self) -> u64 {
        self.observe(1 << 9);
        0.0_f64.to_bits()
    }
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

fn bounded_counts_v1() -> InitialLayerAdmissionCountsV1 {
    InitialLayerAdmissionCountsV1 {
        model_faces: 3,
        material_faces: 3,
        folded_faces: 3,
        overlap_cells: 1,
        directed_orders: 1,
        tested_pairs: 3,
        pose_hinges: 2,
        source_hinges: 2,
    }
}

fn exact_payload_tracker_v1(
    total_bytes: usize,
    total_byte_limit: usize,
    max_integer_bytes: usize,
) -> InitialLayerExactPayloadPreflightV1 {
    InitialLayerExactPayloadPreflightV1 {
        total_bytes,
        total_byte_limit,
        max_integer_bits: max_integer_bytes.saturating_mul(BITS_PER_BYTE_V1),
        max_integer_bytes,
    }
}

fn valid_positive_sample_observation_v1(
    pair: (FaceId, FaceId),
) -> PersistentFlatStackSampleObservationV1 {
    PersistentFlatStackSampleObservationV1 {
        pair,
        complete_pair_scan: true,
        penetration_free: true,
        authority_pair_authenticated: true,
        direct_shared_hinge_authenticated: true,
        hinge_is_stationary: true,
        initial_hinge_angle_bits: Some(180.0_f64.to_bits()),
        current_hinge_angle_bits: Some(180.0_f64.to_bits()),
        topology: TopologyRelation::SharedHingeEdge,
        evidence: IntersectionEvidenceV2::SharedFeatureFlatStack,
        disposition: StaticCollisionPairDisposition::Indeterminate,
    }
}

fn single_face_model_v1() -> MaterialTreeKinematicsModel {
    let vertices = [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)]
        .into_iter()
        .map(|(x, y)| Vertex {
            id: VertexId::new(),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let edges = (0..boundary.len())
        .map(|index| Edge {
            id: EdgeId::new(),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect();
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: ProjectId::new(),
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.issues.is_empty(), "{:#?}", report.issues);
    MaterialTreeKinematicsModel::prepare(
        &pattern,
        &paper,
        &report.snapshot.expect("one-face topology"),
        TreeKinematicsLimits::default(),
    )
    .expect("one-face material tree")
}

#[test]
fn positive_sample_persistent_flat_stack_rejects_each_failed_strict_condition() {
    let valid = valid_positive_sample_observation_v1((FaceId::new(), FaceId::new()));
    assert!(persistent_flat_stack_sample_observation_is_admissible_v1(
        valid
    ));
    let invalid = [
        PersistentFlatStackSampleObservationV1 {
            complete_pair_scan: false,
            ..valid
        },
        PersistentFlatStackSampleObservationV1 {
            penetration_free: false,
            ..valid
        },
        PersistentFlatStackSampleObservationV1 {
            authority_pair_authenticated: false,
            ..valid
        },
        PersistentFlatStackSampleObservationV1 {
            direct_shared_hinge_authenticated: false,
            ..valid
        },
        PersistentFlatStackSampleObservationV1 {
            hinge_is_stationary: false,
            ..valid
        },
        PersistentFlatStackSampleObservationV1 {
            initial_hinge_angle_bits: Some(180.0_f64.to_bits() - 1),
            ..valid
        },
        PersistentFlatStackSampleObservationV1 {
            current_hinge_angle_bits: Some(180.0_f64.to_bits() - 1),
            ..valid
        },
        PersistentFlatStackSampleObservationV1 {
            topology: TopologyRelation::SharedVertex,
            ..valid
        },
        PersistentFlatStackSampleObservationV1 {
            evidence: IntersectionEvidenceV2::Indeterminate,
            ..valid
        },
        PersistentFlatStackSampleObservationV1 {
            disposition: StaticCollisionPairDisposition::Penetrating,
            ..valid
        },
    ];
    for observation in invalid {
        assert!(
            !persistent_flat_stack_sample_observation_is_admissible_v1(observation),
            "every strict positive-sample condition is independently mandatory: \
                 {observation:?}"
        );
    }
}

#[test]
fn zero_sample_requires_the_retained_pose_instance_not_only_equal_angles() {
    let model = single_face_model_v1();
    let angles = CanonicalHingeAngles::new(Vec::new()).expect("empty hinge vector");
    let initial = model.solve(None, &angles).expect("initial pose");
    let cloned = initial.clone();
    let separately_issued = model
        .solve(None, &angles)
        .expect("same-angle independently issued pose");

    assert!(initial_layer_zero_sample_pose_matches_v1(&cloned, &initial));
    assert_eq!(separately_issued.hinge_angles(), initial.hinge_angles());
    assert!(!separately_issued.same_instance(&initial));
    assert!(
        !initial_layer_zero_sample_pose_matches_v1(&separately_issued, &initial),
        "sample zero cannot reuse an equal-angle pose issued outside the retained snapshot"
    );
}

#[test]
fn nondirect_source_ordered_flat_pair_is_initial_only_and_reports_positive_reason() {
    // A current valid Tree cannot produce this static row: shared-hinge
    // flat-stack evidence also supplies one direct Tree hinge. This pure
    // boundary regression deliberately preserves the defensive behavior
    // if a future static classifier broadens that evidence class.
    let first = FaceId::new();
    let second = FaceId::new();
    let expected_pair = initial_layer_canonical_pair_v1(first, second);
    let initial = classify_initial_layer_pair_admission_v1(
        (second, first),
        true,
        StaticCollisionPairDisposition::Indeterminate,
        IntersectionEvidenceV2::SharedFeatureFlatStack,
        None,
        None,
    );
    assert_eq!(initial.pair, expected_pair);
    assert_eq!(
        initial.kind,
        InitialLayerPairAdmissionKindV1::InitialOnlyFlatStack
    );

    let rejection =
        diagnose_nondirect_positive_flat_stack_for_test_v1((second, first)).unwrap_err();
    assert_eq!(rejection.pair, expected_pair);
    assert_eq!(
        rejection.reason,
        PersistentFlatStackSampleRejectionReasonV1::MissingDirectSharedHinge
    );
}

#[test]
fn exact_payload_byte_limits_are_inclusive_and_fail_one_over() {
    let one = exact_integer_v1(1);
    let zero = exact_integer_v1(0);
    let hard_limit = ori_foldability::DEFAULT_MAX_CERTIFICATE_BYTES;

    let mut at_hard_limit = exact_payload_tracker_v1(hard_limit - 1, hard_limit, 1);
    assert_eq!(at_hard_limit.charge_rational(&zero), Ok(()));
    assert_eq!(at_hard_limit.total_bytes, hard_limit);
    let mut one_over_hard = exact_payload_tracker_v1(hard_limit, hard_limit, 1);
    assert_eq!(
        one_over_hard.charge_rational(&zero),
        Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)
    );

    let limits = StaticCollisionLimits {
        max_rational_input_bits: 8,
        max_total_rational_input_storage_bits: 16,
        ..StaticCollisionLimits::default()
    };
    let mut at_limits_cap = InitialLayerExactPayloadPreflightV1::new(limits);
    assert_eq!(at_limits_cap.total_byte_limit, 2);
    assert_eq!(at_limits_cap.charge_rational(&one), Ok(()));
    assert_eq!(at_limits_cap.total_bytes, 2);
    assert_eq!(
        at_limits_cap.charge_rational(&zero),
        Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)
    );
}

#[test]
fn exact_payload_overflow_and_oversized_integer_fail_before_conversion() {
    let zero = exact_integer_v1(0);
    let mut overflow = exact_payload_tracker_v1(usize::MAX, usize::MAX, 1);
    assert_eq!(
        overflow.charge_rational(&zero),
        Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)
    );

    let limits = StaticCollisionLimits::default();
    let mut oversized = InitialLayerExactPayloadPreflightV1::new(limits);
    let huge = ExactRationalValue {
        sign: ExactSign::Positive,
        numerator_magnitude_be: vec![1; oversized.max_integer_bytes + 1],
        denominator_be: vec![1],
    };
    assert_eq!(
        oversized.charge_rational(&huge),
        Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)
    );
    assert_eq!(oversized.total_bytes, 0);
}

#[test]
fn malformed_exact_slices_fail_closed_before_being_charged() {
    let mut tracker = InitialLayerExactPayloadPreflightV1::new(StaticCollisionLimits::default());
    let malformed = [
        ExactRationalValue {
            denominator_be: Vec::new(),
            ..exact_integer_v1(1)
        },
        ExactRationalValue {
            denominator_be: vec![0],
            ..exact_integer_v1(1)
        },
        ExactRationalValue {
            denominator_be: vec![0, 1],
            ..exact_integer_v1(1)
        },
        ExactRationalValue {
            numerator_magnitude_be: vec![0, 0],
            ..exact_integer_v1(0)
        },
        ExactRationalValue {
            numerator_magnitude_be: vec![0],
            denominator_be: vec![2],
            ..exact_integer_v1(0)
        },
        ExactRationalValue {
            sign: ExactSign::Zero,
            ..exact_integer_v1(1)
        },
        ExactRationalValue {
            sign: ExactSign::Positive,
            ..exact_integer_v1(0)
        },
        ExactRationalValue {
            sign: ExactSign::Positive,
            numerator_magnitude_be: vec![0],
            denominator_be: vec![1],
        },
    ];
    for value in malformed {
        assert_eq!(
            tracker.charge_rational(&value),
            Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderUnavailable)
        );
        assert_eq!(tracker.total_bytes, 0);
    }
}

#[test]
fn both_live_zero_numerator_encodings_are_accepted() {
    let mut tracker = InitialLayerExactPayloadPreflightV1::new(StaticCollisionLimits::default());
    let empty_numerator = exact_integer_v1(0);
    let single_zero_numerator = ExactRationalValue {
        numerator_magnitude_be: vec![0],
        ..exact_integer_v1(0)
    };
    assert_eq!(tracker.charge_rational(&empty_numerator), Ok(()));
    assert_eq!(tracker.total_bytes, 1);
    assert_eq!(tracker.charge_rational(&single_zero_numerator), Ok(()));
    assert_eq!(tracker.total_bytes, 3);
}

#[test]
fn canonical_transform_and_exact_boundary_charge_every_component() {
    let mut tracker = InitialLayerExactPayloadPreflightV1::new(StaticCollisionLimits::default());
    tracker
        .charge_transform(&exact_identity_v1())
        .expect("canonical exact transform");
    tracker
        .charge_boundary(&[ExactPointValue {
            x: exact_integer_v1(1),
            y: exact_integer_v1(0),
        }])
        .expect("canonical exact boundary");
    assert_eq!(tracker.total_bytes, 11);
}

#[test]
fn initial_layer_admission_resource_preflight_is_inclusive_and_rejects_pair_one_over() {
    let exact_limits = StaticCollisionLimits {
        max_faces: 3,
        max_unordered_face_pairs: 3,
        ..StaticCollisionLimits::default()
    };
    assert_eq!(
        preflight_initial_layer_admission_counts_v1(bounded_counts_v1(), exact_limits),
        Ok(3)
    );
    assert_eq!(
        preflight_initial_layer_admission_counts_v1(
            bounded_counts_v1(),
            StaticCollisionLimits {
                max_unordered_face_pairs: 2,
                ..exact_limits
            },
        ),
        Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)
    );

    let face_count = 318;
    let expected_pairs = face_count * (face_count - 1) / 2;
    assert_eq!(
        preflight_initial_layer_admission_counts_v1(
            InitialLayerAdmissionCountsV1 {
                model_faces: face_count,
                material_faces: face_count,
                folded_faces: face_count,
                overlap_cells: 0,
                directed_orders: 0,
                tested_pairs: expected_pairs,
                pose_hinges: face_count - 1,
                source_hinges: face_count - 1,
            },
            StaticCollisionLimits {
                max_faces: usize::MAX,
                max_unordered_face_pairs: usize::MAX,
                ..StaticCollisionLimits::default()
            },
        ),
        Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)
    );
    assert_eq!(
        preflight_initial_layer_admission_counts_v1(
            InitialLayerAdmissionCountsV1 {
                model_faces: usize::MAX,
                material_faces: 0,
                folded_faces: 0,
                overlap_cells: 0,
                directed_orders: 0,
                tested_pairs: 0,
                pose_hinges: 0,
                source_hinges: 0,
            },
            StaticCollisionLimits {
                max_faces: usize::MAX,
                max_unordered_face_pairs: usize::MAX,
                ..StaticCollisionLimits::default()
            },
        ),
        Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)
    );
}

#[test]
fn initial_layer_admission_allocation_failure_is_fail_closed() {
    let mut values = Vec::<u8>::new();
    assert_eq!(
        initial_layer_resource_limit_v1(values.try_reserve(usize::MAX)),
        Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)
    );
}

#[test]
fn initial_layer_source_is_captured_once_before_validation() {
    let face = FaceId::new();
    let source = ReadOnceSourceV1 {
        face,
        transform: exact_identity_v1(),
        calls: Cell::new(0),
        cancel_after_observation: None,
    };
    let (captured, expected_pairs) = capture_initial_layer_source_v1(
        &source,
        1,
        0,
        StaticCollisionLimits::default(),
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("bounded one-face source snapshot");
    assert_eq!(expected_pairs, 0);
    assert_eq!(captured.material_faces, vec![face]);
    assert_eq!(captured.folded_faces.len(), 1);
    assert_eq!(captured.fixed_face, Some(face));
    assert_eq!(source.calls.get(), (1 << 10) - 1);
}

#[test]
fn controlled_capture_distinguishes_pre_cancel_and_deadline_without_reading_source() {
    let face = FaceId::new();
    let cancelled = Arc::new(AtomicBool::new(true));
    let source = ReadOnceSourceV1 {
        face,
        transform: exact_identity_v1(),
        calls: Cell::new(0),
        cancel_after_observation: None,
    };
    assert!(matches!(
        capture_initial_layer_source_v1(
            &source,
            1,
            0,
            StaticCollisionLimits::default(),
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(StackedFoldPathDiagnosticErrorV1::Cancelled)
    ));
    assert_eq!(source.calls.get(), 0);

    let deadline_source = ReadOnceSourceV1 {
        face,
        transform: exact_identity_v1(),
        calls: Cell::new(0),
        cancel_after_observation: None,
    };
    assert!(matches!(
        capture_initial_layer_source_v1(
            &deadline_source,
            1,
            0,
            StaticCollisionLimits::default(),
            &CooperativeOperationControlV1::new(None, Instant::now()),
        ),
        Err(StackedFoldPathDiagnosticErrorV1::DeadlineExceeded)
    ));
    assert_eq!(deadline_source.calls.get(), 0);
}

#[test]
fn controlled_capture_stops_after_an_in_progress_source_observation() {
    let face = FaceId::new();
    let cancelled = Arc::new(AtomicBool::new(false));
    let source = ReadOnceSourceV1 {
        face,
        transform: exact_identity_v1(),
        calls: Cell::new(0),
        cancel_after_observation: Some((Arc::clone(&cancelled), 1 << 1)),
    };
    assert!(matches!(
        capture_initial_layer_source_v1(
            &source,
            1,
            0,
            StaticCollisionLimits::default(),
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(StackedFoldPathDiagnosticErrorV1::Cancelled)
    ));
    assert_eq!(source.calls.get() & (1 << 1), 1 << 1);
    assert_eq!(source.calls.get() & (1 << 3), 0);
}

#[test]
fn controlled_pair_order_validation_stops_before_partial_result() {
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        validate_initial_layer_order_dag_v1(
            3,
            &[(0, 1), (1, 2)],
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(StackedFoldPathDiagnosticErrorV1::Cancelled)
    );
}

#[test]
fn oversized_exact_source_fails_during_the_single_capture() {
    let face = FaceId::new();
    let mut transform = exact_identity_v1();
    let integer_limit_bytes = ori_foldability::DEFAULT_MAX_EXACT_INTEGER_BITS / BITS_PER_BYTE_V1;
    transform.m00 = ExactRationalValue {
        sign: ExactSign::Positive,
        numerator_magnitude_be: vec![1; integer_limit_bytes + 1],
        denominator_be: vec![1],
    };
    let source = ReadOnceSourceV1 {
        face,
        transform,
        calls: Cell::new(0),
        cancel_after_observation: None,
    };
    assert!(matches!(
        capture_initial_layer_source_v1(
            &source,
            1,
            0,
            StaticCollisionLimits {
                max_rational_input_bits: usize::MAX,
                max_total_rational_input_storage_bits: usize::MAX,
                ..StaticCollisionLimits::default()
            },
            &CooperativeOperationControlV1::unbounded(),
        ),
        Err(StackedFoldPathDiagnosticErrorV1::InitialLayerOrderResourceLimit)
    ));
    assert_eq!(source.calls.get(), 383);
}

fn canonical_face_pair_array_v1(first: FaceId, second: FaceId) -> [FaceId; 2] {
    let (first, second) = initial_layer_canonical_pair_v1(first, second);
    [first, second]
}

type ThreeStationaryTransportAuthorityFixtureV1 = (
    [(EdgeId, [FaceId; 2]); 3],
    [PersistentFlatHingeAdmissionV1; 3],
    [NonFlatFacePairOrderStructuralV1; 3],
);

fn three_stationary_transport_authority_v1() -> ThreeStationaryTransportAuthorityFixtureV1 {
    let pairs = [
        canonical_face_pair_array_v1(FaceId::new(), FaceId::new()),
        canonical_face_pair_array_v1(FaceId::new(), FaceId::new()),
        canonical_face_pair_array_v1(FaceId::new(), FaceId::new()),
    ];
    let expected = [
        (EdgeId::new(), pairs[0]),
        (EdgeId::new(), pairs[1]),
        (EdgeId::new(), pairs[2]),
    ];
    let persistent = expected.map(|(hinge, pair)| PersistentFlatHingeAdmissionV1 {
        first_face: pair[0],
        second_face: pair[1],
        hinge,
    });
    let directed = pairs.map(|pair| NonFlatFacePairOrderStructuralV1 {
        lower_face: pair[0],
        upper_face: pair[1],
    });
    (expected, persistent, directed)
}

fn bind_three_stationary_authorities_v1(
    persistent: &[PersistentFlatHingeAdmissionV1],
    directed: &[NonFlatFacePairOrderStructuralV1],
    expected: &[(EdgeId, [FaceId; 2])],
) -> Result<
    Option<Vec<StationaryFlatStackTransportBindingV1>>,
    StationaryFlatStackTransportBindingErrorV1,
> {
    stationary_flat_stack_transport_bindings_from_authority_with_control_v1(
        persistent,
        directed,
        expected,
        3,
        &CooperativeOperationControlV1::unbounded(),
    )
}

#[test]
fn three_stationary_transport_requires_exact_missing_and_extra_authority_coverage() {
    let (expected, persistent, directed) = three_stationary_transport_authority_v1();
    let bindings = bind_three_stationary_authorities_v1(&persistent, &directed, &expected)
        .expect("bounded authority matching")
        .expect("complete three-stationary authority");
    assert_eq!(bindings.len(), 3);

    assert_eq!(
        bind_three_stationary_authorities_v1(&persistent[..2], &directed, &expected),
        Ok(None),
        "a missing retained hinge must fail closed"
    );
    assert_eq!(
        bind_three_stationary_authorities_v1(&persistent, &directed[..2], &expected),
        Ok(None),
        "a missing directed order must fail closed"
    );

    let mut extra_persistent = persistent.to_vec();
    extra_persistent.push(PersistentFlatHingeAdmissionV1 {
        first_face: FaceId::new(),
        second_face: FaceId::new(),
        hinge: EdgeId::new(),
    });
    assert_eq!(
        bind_three_stationary_authorities_v1(&extra_persistent, &directed, &expected),
        Ok(None),
        "an unbound retained hinge must fail closed"
    );
    let mut extra_directed = directed.to_vec();
    extra_directed.push(NonFlatFacePairOrderStructuralV1 {
        lower_face: FaceId::new(),
        upper_face: FaceId::new(),
    });
    assert_eq!(
        bind_three_stationary_authorities_v1(&persistent, &extra_directed, &expected),
        Ok(None),
        "an unbound directed order must fail closed"
    );
}

#[test]
fn three_stationary_transport_rejects_duplicate_and_foreign_authorities() {
    let (expected, persistent, directed) = three_stationary_transport_authority_v1();

    let mut duplicate_expected = expected;
    duplicate_expected[1] = duplicate_expected[0];
    assert_eq!(
        bind_three_stationary_authorities_v1(&persistent, &directed, &duplicate_expected),
        Ok(None)
    );
    let mut duplicate_persistent = persistent;
    duplicate_persistent[1] = duplicate_persistent[0];
    assert_eq!(
        bind_three_stationary_authorities_v1(&duplicate_persistent, &directed, &expected),
        Ok(None)
    );
    let mut duplicate_directed = directed;
    duplicate_directed[1] = duplicate_directed[0];
    assert_eq!(
        bind_three_stationary_authorities_v1(&persistent, &duplicate_directed, &expected),
        Ok(None)
    );

    let mut foreign_persistent = persistent;
    foreign_persistent[1].hinge = EdgeId::new();
    assert_eq!(
        bind_three_stationary_authorities_v1(&foreign_persistent, &directed, &expected),
        Ok(None)
    );
    let mut foreign_directed = directed;
    foreign_directed[1] = NonFlatFacePairOrderStructuralV1 {
        lower_face: FaceId::new(),
        upper_face: FaceId::new(),
    };
    assert_eq!(
        bind_three_stationary_authorities_v1(&persistent, &foreign_directed, &expected),
        Ok(None)
    );
}

#[test]
fn three_stationary_transport_preserves_reversed_directional_authority() {
    let (expected, persistent, mut directed) = three_stationary_transport_authority_v1();
    directed[1] = NonFlatFacePairOrderStructuralV1 {
        lower_face: expected[1].1[1],
        upper_face: expected[1].1[0],
    };
    let bindings = bind_three_stationary_authorities_v1(&persistent, &directed, &expected)
        .expect("bounded authority matching")
        .expect("reversed source direction remains authenticated");
    assert_eq!(bindings[1].pair, expected[1].1);
    assert_eq!(bindings[1].lower_face, expected[1].1[1]);
    assert_eq!(bindings[1].upper_face, expected[1].1[0]);
}

#[test]
fn three_stationary_transport_preserves_resource_and_stop_causes() {
    let (expected, persistent, directed) = three_stationary_transport_authority_v1();
    assert_eq!(
        stationary_flat_stack_transport_bindings_from_authority_with_control_v1(
            &persistent,
            &directed,
            &expected,
            2,
            &CooperativeOperationControlV1::unbounded(),
        ),
        Err(StationaryFlatStackTransportBindingErrorV1::ResourceLimit)
    );

    let cancelled = AtomicBool::new(true);
    assert_eq!(
        stationary_flat_stack_transport_bindings_from_authority_with_control_v1(
            &persistent,
            &directed,
            &expected,
            3,
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(StationaryFlatStackTransportBindingErrorV1::Cancelled)
    );
    assert_eq!(
        stationary_flat_stack_transport_bindings_from_authority_with_control_v1(
            &persistent,
            &directed,
            &expected,
            3,
            &CooperativeOperationControlV1::new(None, Instant::now()),
        ),
        Err(StationaryFlatStackTransportBindingErrorV1::DeadlineExceeded)
    );
}
