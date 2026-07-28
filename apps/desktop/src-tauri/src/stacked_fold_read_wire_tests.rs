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

fn transaction_proposal_fixture_v1() -> StackedFoldTransactionProposalDto {
    StackedFoldTransactionProposalDto {
        apply_contract_version: STACKED_FOLD_APPLY_CONTRACT_VERSION_V1,
        apply_mode: StackedFoldApplyModeDtoV1::None,
        transaction_token: None,
        speculative_unproven_available: false,
        source_project_id: ProjectId::new(),
        source_revision: 3,
        target_revision: 4,
        source_fingerprint_sha256: "11".repeat(32),
        target_fingerprint_sha256: "22".repeat(32),
        added_vertex_count: 1,
        added_edge_count: 2,
        mountain_crease_count: 1,
        valley_crease_count: 1,
        timeline_step_count: 1,
        timeline_complete_hinge_angle_count: 2,
        requested_angle_degrees: 90.0,
        ready_for_atomic_apply: false,
        failure_classes: vec![StackedFoldTransactionFailureClassDto::ContinuousPathUncertified],
        authorizes_project_mutation: false,
    }
}

#[test]
fn transaction_apply_modes_serialize_exactly_and_preserve_authority_separation() {
    let token = ProjectId::new();
    let mut proposal = transaction_proposal_fixture_v1();
    assert!(proposal.has_valid_apply_contract_v1(false, true));
    let none = serde_json::to_value(&proposal).unwrap();
    assert_eq!(none["applyContractVersion"], serde_json::json!(1));
    assert_eq!(none["applyMode"], serde_json::json!("none"));
    assert_eq!(none["transactionToken"], serde_json::Value::Null);
    assert_eq!(
        none["speculativeUnprovenAvailable"],
        serde_json::json!(false)
    );

    proposal.publish_speculative_unproven_v1(token);
    assert!(proposal.has_valid_apply_contract_v1(false, true));
    let speculative = serde_json::to_value(&proposal).unwrap();
    assert_eq!(
        speculative["applyMode"],
        serde_json::json!("speculative_unproven")
    );
    assert_eq!(speculative["transactionToken"], serde_json::json!(token));
    assert_eq!(
        speculative["speculativeUnprovenAvailable"],
        serde_json::json!(true)
    );
    assert_eq!(speculative["readyForAtomicApply"], serde_json::json!(false));
    assert_eq!(
        speculative["authorizesProjectMutation"],
        serde_json::json!(false)
    );
    assert_eq!(
        speculative["failureClasses"],
        serde_json::json!(["continuous_path_uncertified"])
    );

    proposal.publish_certified_v1(token);
    assert!(proposal.has_valid_apply_contract_v1(true, true));
    let certified = serde_json::to_value(&proposal).unwrap();
    assert_eq!(certified["applyMode"], serde_json::json!("certified"));
    assert_eq!(
        certified["speculativeUnprovenAvailable"],
        serde_json::json!(false)
    );
    assert_eq!(certified["readyForAtomicApply"], serde_json::json!(true));
    assert_eq!(
        certified["authorizesProjectMutation"],
        serde_json::json!(true)
    );
}

#[test]
fn transaction_apply_contract_rejects_every_cross_mode_authority_mix() {
    let mut proposal = transaction_proposal_fixture_v1();
    proposal.transaction_token = Some(ProjectId::new());
    assert!(!proposal.has_valid_apply_contract_v1(false, true));

    proposal.apply_mode = StackedFoldApplyModeDtoV1::SpeculativeUnproven;
    proposal.speculative_unproven_available = true;
    assert!(proposal.has_valid_apply_contract_v1(false, true));
    proposal.ready_for_atomic_apply = true;
    assert!(!proposal.has_valid_apply_contract_v1(false, true));
    proposal.ready_for_atomic_apply = false;
    proposal.authorizes_project_mutation = true;
    assert!(!proposal.has_valid_apply_contract_v1(false, true));

    proposal.apply_mode = StackedFoldApplyModeDtoV1::Certified;
    proposal.speculative_unproven_available = false;
    proposal.ready_for_atomic_apply = true;
    assert!(!proposal.has_valid_apply_contract_v1(false, true));
    proposal.failure_classes.clear();
    assert!(proposal.has_valid_apply_contract_v1(true, true));
    proposal.speculative_unproven_available = true;
    assert!(!proposal.has_valid_apply_contract_v1(true, true));

    proposal.publish_certified_v1(ProjectId::new());
    proposal.failure_classes =
        vec![StackedFoldTransactionFailureClassDto::ContinuousPathUncertified];
    assert!(!proposal.has_valid_apply_contract_v1(true, true));
    proposal.publish_speculative_unproven_v1(ProjectId::new());
    proposal.failure_classes.clear();
    assert!(!proposal.has_valid_apply_contract_v1(false, true));

    let mut none = transaction_proposal_fixture_v1();
    none.failure_classes = vec![
        StackedFoldTransactionFailureClassDto::TargetLayerOrderUnavailable,
        StackedFoldTransactionFailureClassDto::ContinuousPathUncertified,
    ];
    assert!(!none.has_valid_apply_contract_v1(false, false));
}

#[test]
fn transaction_failure_classes_must_exactly_match_computed_evidence() {
    for (continuous, layer, expected) in [
        (true, true, Vec::new()),
        (
            false,
            true,
            vec![StackedFoldTransactionFailureClassDto::ContinuousPathUncertified],
        ),
        (
            true,
            false,
            vec![StackedFoldTransactionFailureClassDto::TargetLayerOrderUnavailable],
        ),
        (
            false,
            false,
            vec![
                StackedFoldTransactionFailureClassDto::ContinuousPathUncertified,
                StackedFoldTransactionFailureClassDto::TargetLayerOrderUnavailable,
            ],
        ),
    ] {
        let mut proposal = transaction_proposal_fixture_v1();
        proposal.failure_classes = expected;
        assert!(proposal.has_valid_apply_contract_v1(continuous, layer));

        proposal.failure_classes.reverse();
        if proposal.failure_classes.len() > 1 {
            assert!(!proposal.has_valid_apply_contract_v1(continuous, layer));
        }
        proposal.failure_classes = transaction_failure_classes(continuous, layer);
        proposal
            .failure_classes
            .push(StackedFoldTransactionFailureClassDto::ContinuousPathUncertified);
        assert!(!proposal.has_valid_apply_contract_v1(continuous, layer));
    }

    let proposal = transaction_proposal_fixture_v1();
    assert!(!proposal.has_valid_apply_contract_v1(true, true));
    assert!(!proposal.has_valid_apply_contract_v1(true, false));
    assert!(!proposal.has_valid_apply_contract_v1(false, false));
}

#[test]
fn explicit_half_angle_schedule_uses_graph_proof_boundary_for_tree_topology() {
    assert!(requires_graph_schedule_boundary_v1(false, true));
    assert!(requires_graph_schedule_boundary_v1(true, false));
    assert!(!requires_graph_schedule_boundary_v1(false, false));
}
