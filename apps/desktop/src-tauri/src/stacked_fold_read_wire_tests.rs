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
fn explicit_half_angle_schedule_uses_graph_proof_boundary_for_tree_topology() {
    assert!(requires_graph_schedule_boundary_v1(false, true));
    assert!(requires_graph_schedule_boundary_v1(true, false));
    assert!(!requires_graph_schedule_boundary_v1(false, false));
}
