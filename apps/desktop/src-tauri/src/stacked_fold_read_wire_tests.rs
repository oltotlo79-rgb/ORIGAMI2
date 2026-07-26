use ori_collision::{
    StackedFoldFixedSideV1, StackedFoldLinearCandidateV1, StackedFoldRotationDirectionV1,
};
use ori_domain::ProjectId;
use ori_kinematics::Point3;

use super::*;

#[test]
fn request_schema_is_closed_and_rejects_non_finite_points() {
    let project_instance_id = ProjectId::new();
    let project_id = ProjectId::new();
    let json = serde_json::json!({
        "expectedProjectInstanceId": project_instance_id,
        "expectedProjectId": project_id,
        "expectedRevision": 7,
        "first": [10.0, 0.0, 0.0],
        "second": [10.0, 0.0, -20.0],
        "fixedSide": "left",
        "rotationDirection": "positive",
        "requestedAngleDegrees": 90.0
    });
    let request: StackedFoldReadRequest =
        serde_json::from_value(json.clone()).expect("valid request");
    assert_eq!(request.expected_revision, 7);
    assert!(
        StackedFoldLinearCandidateV1::new(
            Point3::new(request.first[0], request.first[1], request.first[2]).unwrap(),
            Point3::new(request.second[0], request.second[1], request.second[2]).unwrap(),
            request.fixed_side.into(),
            request.rotation_direction.into(),
            request.requested_angle_degrees,
        )
        .is_ok()
    );
    let mut unknown = json.clone();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("future".to_owned(), serde_json::Value::Bool(true));
    assert!(serde_json::from_value::<StackedFoldReadRequest>(unknown).is_err());

    let mut non_finite = json;
    non_finite["first"][0] = serde_json::json!(f64::INFINITY);
    assert!(
        serde_json::from_value::<StackedFoldReadRequest>(non_finite)
            .ok()
            .and_then(|request| {
                Point3::new(request.first[0], request.first[1], request.first[2]).ok()
            })
            .is_none()
    );
}

#[test]
fn request_schema_rejects_missing_malformed_and_open_enum_values() {
    let valid = serde_json::json!({
        "expectedProjectInstanceId": ProjectId::new(),
        "expectedProjectId": ProjectId::new(),
        "expectedRevision": 7,
        "first": [10.0, 0.0, 0.0],
        "second": [10.0, 0.0, -20.0],
        "fixedSide": "left",
        "rotationDirection": "positive",
        "requestedAngleDegrees": 90.0
    });

    for field in [
        "expectedProjectInstanceId",
        "expectedProjectId",
        "expectedRevision",
        "first",
        "second",
        "fixedSide",
        "rotationDirection",
        "requestedAngleDegrees",
    ] {
        let mut missing = valid.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<StackedFoldReadRequest>(missing).is_err(),
            "missing field {field} must be rejected"
        );
    }

    for malformed in [
        ("first", serde_json::json!([10.0, 0.0])),
        ("second", serde_json::json!([10.0, 0.0, -20.0, 1.0])),
        ("fixedSide", serde_json::json!("center")),
        ("fixedSide", serde_json::json!("Left")),
        ("rotationDirection", serde_json::json!("clockwise")),
        ("rotationDirection", serde_json::json!("Positive")),
    ] {
        let mut request = valid.clone();
        request[malformed.0] = malformed.1;
        assert!(
            serde_json::from_value::<StackedFoldReadRequest>(request).is_err(),
            "malformed field {} must be rejected",
            malformed.0
        );
    }
}

#[test]
fn candidate_validation_rejects_degenerate_line_and_invalid_angles() {
    let point = Point3::new(1.0, 2.0, 3.0).unwrap();
    assert!(
        StackedFoldLinearCandidateV1::new(
            point,
            point,
            StackedFoldFixedSideV1::Left,
            StackedFoldRotationDirectionV1::Positive,
            90.0,
        )
        .is_err()
    );

    let other = Point3::new(2.0, 2.0, 3.0).unwrap();
    for angle in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.0,
        -90.0,
        180.1,
    ] {
        assert!(
            StackedFoldLinearCandidateV1::new(
                point,
                other,
                StackedFoldFixedSideV1::Right,
                StackedFoldRotationDirectionV1::Negative,
                angle,
            )
            .is_err(),
            "invalid angle {angle:?} must be rejected"
        );
    }
}

#[test]
fn transaction_proposal_failure_classes_are_explicit_and_fail_closed() {
    let missing_all = serde_json::to_value(transaction_failure_classes(false, false)).unwrap();
    assert_eq!(
        missing_all,
        serde_json::json!([
            "continuous_path_uncertified",
            "target_layer_order_unavailable"
        ])
    );
    let ready = serde_json::to_value(transaction_failure_classes(true, true)).unwrap();
    assert_eq!(ready, serde_json::json!([]));
}

#[test]
fn cycle_schedule_wire_rejects_unknown_fields_and_numeric_overflow() {
    let request = || {
        serde_json::json!({
            "expectedProjectInstanceId": "018f47a2-4b7a-7cc1-8abc-112233445566",
            "expectedProjectId": "018f47a2-4b7a-7cc1-8abc-665544332211",
            "expectedRevision": 3,
            "first": [0.0, 0.0, 0.0],
            "second": [1.0, 0.0, 0.0],
            "fixedSide": "left",
            "rotationDirection": "positive",
            "requestedAngleDegrees": 90.0,
            "cycleScheduleV1": {
                "version": 1,
                "entries": [{
                    "edge": "018f47a2-4b7a-7cc1-8abc-778899aabbcc",
                    "uDomain": [
                        {"numerator": 0, "denominator": 1},
                        {"numerator": 1, "denominator": 1}
                    ],
                    "numeratorPowerCoefficients": [{"numerator": 1, "denominator": 1}],
                    "denominatorPowerCoefficients": [{"numerator": 1, "denominator": 1}],
                    "requestedAngleDegrees": 90.0
                }]
            }
        })
    };
    let admitted = serde_json::from_value::<StackedFoldReadRequest>(request()).unwrap();
    assert_eq!(validate_request_resource_shape_v1(&admitted), Ok(()));
    let mut unknown = request();
    unknown["cycleScheduleV1"]["entries"][0]["authority"] = serde_json::json!(true);
    assert!(serde_json::from_value::<StackedFoldReadRequest>(unknown).is_err());
    let mut overflow = request();
    overflow["cycleScheduleV1"]["entries"][0]["uDomain"][0]["denominator"] = serde_json::json!(-1);
    assert!(serde_json::from_value::<StackedFoldReadRequest>(overflow).is_err());

    let mut coefficient_exhaustion = request();
    coefficient_exhaustion["cycleScheduleV1"]["entries"][0]["numeratorPowerCoefficients"] = serde_json::json!(
        (0..=MAX_CYCLE_SCHEDULE_COEFFICIENTS_V1)
            .map(|_| serde_json::json!({"numerator": 1, "denominator": 1}))
            .collect::<Vec<_>>()
    );
    let coefficient_exhaustion =
        serde_json::from_value::<StackedFoldReadRequest>(coefficient_exhaustion).unwrap();
    assert_eq!(
        validate_request_resource_shape_v1(&coefficient_exhaustion),
        Err(CYCLE_PATH_RESOURCE_MESSAGE)
    );
}

#[test]
fn explicit_half_angle_schedule_uses_graph_proof_boundary_for_tree_topology() {
    assert!(requires_graph_schedule_boundary_v1(false, true));
    assert!(requires_graph_schedule_boundary_v1(true, false));
    assert!(!requires_graph_schedule_boundary_v1(false, false));
}
