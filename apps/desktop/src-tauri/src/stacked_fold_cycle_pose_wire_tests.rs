use super::*;

#[test]
fn linear_candidate_requires_bit_exact_live_initial_angles() {
    let edge = serde_json::from_value::<ori_domain::EdgeId>(serde_json::json!(
        "018f47a2-4b7a-7cc1-8abc-778899aabbcc"
    ))
    .unwrap();
    let live = ori_kinematics::CanonicalHingeAngles::new(vec![
        ori_kinematics::HingeAngle::new(edge, 20.0).unwrap(),
    ])
    .unwrap();
    let request = LinearCandidateRequestV1 {
        version: 1,
        exact_dyadic_path_v1: None,
        entries: vec![LinearCandidateEntryRequestV1 {
            edge,
            initial_angle_degrees: 20.0,
            requested_angle_degrees: 40.0,
        }],
    };
    let (initial, requested) = validate_linear_candidate_angles_v1(&request, &live).unwrap();
    assert_eq!(initial, live);
    assert_ne!(requested, live);

    let mismatch = LinearCandidateRequestV1 {
        version: 1,
        exact_dyadic_path_v1: None,
        entries: vec![LinearCandidateEntryRequestV1 {
            edge,
            initial_angle_degrees: f64::from_bits(20.0f64.to_bits() + 1),
            requested_angle_degrees: 40.0,
        }],
    };
    assert!(validate_linear_candidate_angles_v1(&mismatch, &live).is_err());
    let wrong_version = LinearCandidateRequestV1 {
        version: 2,
        exact_dyadic_path_v1: None,
        entries: request.entries,
    };
    assert!(validate_linear_candidate_angles_v1(&wrong_version, &live).is_err());
}

#[test]
fn exact_dyadic_candidate_preflight_rejects_crossing_and_allows_endpoint_touch() {
    let point = |x, y, power| ExactDyadicPointRequestV1 {
        x_numerator: x,
        y_numerator: y,
        denominator_power: power,
    };
    let path = |second| ExactDyadicPathRequestV1 {
        version: 1,
        segments: vec![
            ExactDyadicSegmentRequestV1 {
                start: point(0, 0, 0),
                end: point(2, 0, 0),
            },
            second,
        ],
        max_pair_tests: 1,
        max_denominator_power: 80,
        max_integer_bits: 256,
    };
    assert_eq!(
        validate_exact_dyadic_candidate_path_v1(&path(ExactDyadicSegmentRequestV1 {
            start: point(1, -1, 80),
            end: point(1, 1, 80),
        })),
        Err(CYCLE_PATH_UNCERTIFIED_MESSAGE)
    );
    assert_eq!(
        validate_exact_dyadic_candidate_path_v1(&path(ExactDyadicSegmentRequestV1 {
            start: point(2, 0, 0),
            end: point(3, 1, 0),
        })),
        Ok(())
    );
    let mut bounded = path(ExactDyadicSegmentRequestV1 {
        start: point(1, 1, 80),
        end: point(1, 2, 80),
    });
    bounded.max_pair_tests = 0;
    assert_eq!(
        validate_exact_dyadic_candidate_path_v1(&bounded),
        Err(CYCLE_PATH_RESOURCE_MESSAGE)
    );
}

#[test]
fn certified_path_graph_admission_is_live_bound_canonical_and_bounded() {
    let edge = ori_domain::EdgeId::new();
    let live = ori_kinematics::CanonicalHingeAngles::new(vec![
        ori_kinematics::HingeAngle::new(edge, 0.0).unwrap(),
    ])
    .unwrap();
    let state = |angle_degrees| CertifiedPathGraphStateRequestV1 {
        entries: vec![CertifiedPathGraphAngleRequestV1 {
            edge,
            angle_degrees,
        }],
    };
    let valid = CertifiedPathGraphRequestV1 {
        version: 1,
        states: vec![state(0.0), state(45.0), state(90.0)],
        transitions: vec![
            CertifiedPathGraphTransitionRequestV1 {
                source_state: 0,
                target_state: 1,
            },
            CertifiedPathGraphTransitionRequestV1 {
                source_state: 1,
                target_state: 2,
            },
        ],
        source_state: 0,
        target_state: 2,
    };
    assert_eq!(
        validate_certified_path_graph_v1(&valid, &live)
            .unwrap()
            .len(),
        3
    );

    let stale = CertifiedPathGraphRequestV1 {
        states: vec![state(1.0), state(45.0)],
        target_state: 1,
        transitions: vec![CertifiedPathGraphTransitionRequestV1 {
            source_state: 0,
            target_state: 1,
        }],
        ..valid
    };
    assert_eq!(
        validate_certified_path_graph_v1(&stale, &live),
        Err(CYCLE_PATH_UNSUPPORTED_MESSAGE)
    );
    let over_limit = CertifiedPathGraphRequestV1 {
        version: 1,
        states: (0..=ori_collision::MAX_CERTIFIED_PATH_GRAPH_STATES_V1)
            .map(|index| state(index as f64))
            .collect(),
        target_state: 1,
        transitions: Vec::new(),
        source_state: 0,
    };
    assert_eq!(
        validate_certified_path_graph_v1(&over_limit, &live),
        Err(CYCLE_PATH_RESOURCE_MESSAGE)
    );
    let transition_over_limit = CertifiedPathGraphRequestV1 {
        version: 1,
        states: vec![state(0.0), state(90.0)],
        transitions: (0..=MAX_STACKED_FOLD_ATOMIC_PATH_TRANSITIONS_V1)
            .map(|_| CertifiedPathGraphTransitionRequestV1 {
                source_state: 0,
                target_state: 1,
            })
            .collect(),
        source_state: 0,
        target_state: 1,
    };
    assert_eq!(
        validate_certified_path_graph_v1(&transition_over_limit, &live),
        Err(CYCLE_PATH_RESOURCE_MESSAGE)
    );
    let oversized_state = CertifiedPathGraphRequestV1 {
        version: 1,
        states: vec![
            CertifiedPathGraphStateRequestV1 {
                entries: (0..=MAX_STACKED_FOLD_REQUEST_HINGES_V1)
                    .map(|_| CertifiedPathGraphAngleRequestV1 {
                        edge: ori_domain::EdgeId::new(),
                        angle_degrees: 0.0,
                    })
                    .collect(),
            },
            state(90.0),
        ],
        transitions: vec![CertifiedPathGraphTransitionRequestV1 {
            source_state: 0,
            target_state: 1,
        }],
        source_state: 0,
        target_state: 1,
    };
    assert_eq!(
        validate_certified_path_graph_v1(&oversized_state, &live),
        Err(CYCLE_PATH_RESOURCE_MESSAGE)
    );
}

#[test]
fn current_cycle_preview_request_rejects_unknown_dto_fields() {
    let id = ProjectId::new();
    let value = serde_json::json!({
        "expectedProjectInstanceId": id,
        "expectedProjectId": id,
        "expectedRevision": 0,
        "cycleScheduleV1": { "version": 1, "entries": [] },
        "unexpected": true
    });
    assert!(serde_json::from_value::<CurrentCyclePosePreviewRequestV1>(value).is_err());
}

#[test]
fn current_cycle_progress_id_is_strict_and_bounded() {
    assert_eq!(validate_progress_request_id_v1(None).unwrap(), None);
    assert_eq!(
        validate_progress_request_id_v1(Some("cycle:1")).unwrap(),
        Some("cycle:1")
    );
    assert!(validate_progress_request_id_v1(Some("")).is_err());
    assert!(validate_progress_request_id_v1(Some(&"x".repeat(129))).is_err());
    assert!(validate_progress_request_id_v1(Some("循環")).is_err());
}
