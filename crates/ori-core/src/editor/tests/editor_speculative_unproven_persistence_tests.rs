use serde_json::{Value, json};

use super::{speculative_unproven_test_support::*, *};

#[test]
fn trimmed_applied_mark_survives_round_trip_with_terminal_reason() {
    let mut fixture = fixture();
    fixture
        .editor
        .set_history_entry_limit(1)
        .expect("minimum retained history");
    let binding = binding(
        &fixture,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut fixture, binding.clone());
    for memo in ["one", "two", "three"] {
        fixture
            .editor
            .execute(
                fixture.editor.revision(),
                Command::UpdateProjectMemo {
                    memo: memo.to_owned(),
                },
            )
            .expect("trim-inducing edit");
    }
    let report = fixture
        .editor
        .resolve_speculative_unproven_fold_v1(
            &binding,
            SpeculativeUnprovenFoldProofOutcomeV1::Unknown {
                reason: SpeculativeUnprovenFoldUnknownReasonV1::ResourceLimit,
            },
        )
        .expect("resolve trimmed mark");
    assert_eq!(
        report.location,
        SpeculativeUnprovenFoldHistoryLocationV1::AppliedTrimmedBase
    );
    assert_eq!(report.subsequent_edit_count, 3);
    assert_eq!(report.undo_steps_to_revert, None);

    let history = fixture
        .editor
        .export_history_v1(fixture.project_id)
        .expect("export speculative history");
    assert!(history.requires_speculative_unproven_fold_feature_v1());
    let json = serde_json::to_value(&history).expect("history JSON");
    assert!(json.get("speculative_unproven_applied_base_v1").is_some());
    let reopened = reopen(&fixture.editor, history).expect("reopen speculative history");
    assert_eq!(
        reopened.speculative_unproven_fold_summary_v1(),
        fixture.editor.speculative_unproven_fold_summary_v1()
    );
    assert_eq!(
        reopened
            .speculative_unproven_fold_summary_v1()
            .applied
            .unknown_resource_limit,
        1
    );
}

#[test]
fn unapplied_redo_mark_round_trips_and_reapplies() {
    let mut fixture = fixture();
    let binding = binding(
        &fixture,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut fixture, binding);
    fixture.editor.undo(1).expect("undo marked entry");
    let history = fixture
        .editor
        .export_history_v1(fixture.project_id)
        .expect("export Redo mark");
    let mut reopened = reopen(&fixture.editor, history).expect("restore Redo mark");
    assert_eq!(
        reopened
            .speculative_unproven_fold_summary_v1()
            .unapplied_redo
            .awaiting_proof,
        1
    );
    reopened.redo(0).expect("redo restored marked entry");
    assert_eq!(
        reopened
            .speculative_unproven_fold_summary_v1()
            .applied
            .awaiting_proof,
        1
    );
}

#[test]
fn tampered_applied_base_depth_is_rejected() {
    let mut fixture = fixture();
    fixture
        .editor
        .set_history_entry_limit(1)
        .expect("minimum history");
    let binding = binding(
        &fixture,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut fixture, binding);
    fixture
        .editor
        .execute(
            fixture.editor.revision(),
            Command::UpdateProjectMemo {
                memo: "trim mark".to_owned(),
            },
        )
        .expect("trim marked entry");
    let mut json = serde_json::to_value(
        fixture
            .editor
            .export_history_v1(fixture.project_id)
            .expect("export base ledger"),
    )
    .expect("history JSON");
    json["speculative_unproven_applied_base_v1"]["retained_marks"][0]["subsequent_applied_entries"] =
        json!(0);
    let history = serde_json::from_value::<EditorHistoryV1>(json).expect("well-shaped history");
    assert!(matches!(
        reopen(&fixture.editor, history),
        Err(EditorHistoryErrorV1::InvalidSpeculativeAppliedBaseLedger)
    ));
}

#[test]
fn speculative_wire_tampering_is_fail_closed_at_every_boundary() {
    let mut fixture = fixture();
    let binding = binding(
        &fixture,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut fixture, binding);
    let history = fixture
        .editor
        .export_history_v1(fixture.project_id)
        .expect("export marked history");
    let original = serde_json::to_value(&history).expect("history JSON");

    let mut unknown = original.clone();
    unknown["undo_stack"][0]["speculative_unproven_fold_v1"]["unexpected"] = json!(true);
    assert!(serde_json::from_value::<EditorHistoryV1>(unknown).is_err());

    let mut foreign_project = original.clone();
    foreign_project["undo_stack"][0]["speculative_unproven_fold_v1"]["binding"]["project_id"] =
        json!(ProjectId::new());
    let decoded =
        serde_json::from_value::<EditorHistoryV1>(foreign_project).expect("well-shaped history");
    assert!(matches!(
        reopen(&fixture.editor, decoded),
        Err(EditorHistoryErrorV1::InvalidSpeculativeUnprovenMetadata)
    ));

    let mut stale_fingerprint = original.clone();
    stale_fingerprint["undo_stack"][0]["speculative_unproven_fold_v1"]["binding"]["source_geometry_fingerprint_sha256"] =
        json!("0".repeat(64));
    let decoded =
        serde_json::from_value::<EditorHistoryV1>(stale_fingerprint).expect("well-shaped history");
    assert!(matches!(
        reopen(&fixture.editor, decoded),
        Err(EditorHistoryErrorV1::InvalidSpeculativeUnprovenMetadata)
    ));

    let mut blocking = original.clone();
    blocking["undo_stack"][0]["speculative_unproven_fold_v1"]["binding"]["approximate_blocking_observation"] = json!({
        "status": "blocking_sample_observed",
        "first_blocking_angle_bits_be": 45.0_f64.to_bits().to_be_bytes(),
    });
    let decoded = serde_json::from_value::<EditorHistoryV1>(blocking).expect("well-shaped history");
    assert!(matches!(
        reopen(&fixture.editor, decoded),
        Err(EditorHistoryErrorV1::InvalidSpeculativeUnprovenMetadata)
    ));

    fixture
        .editor
        .execute(
            fixture.editor.revision(),
            Command::UpdateProjectMemo {
                memo: "ordinary".to_owned(),
            },
        )
        .expect("ordinary entry");
    let mut ordinary_entry = serde_json::to_value(
        fixture
            .editor
            .export_history_v1(fixture.project_id)
            .expect("two-entry history"),
    )
    .expect("history JSON");
    ordinary_entry["undo_stack"][1]["speculative_unproven_fold_v1"] =
        original["undo_stack"][0]["speculative_unproven_fold_v1"].clone();
    let decoded =
        serde_json::from_value::<EditorHistoryV1>(ordinary_entry).expect("well-shaped history");
    assert!(matches!(
        reopen(&fixture.editor, decoded),
        Err(EditorHistoryErrorV1::InvalidSpeculativeUnprovenMetadata
            | EditorHistoryErrorV1::DuplicateSpeculativeBinding)
    ));
}

#[test]
fn speculative_mark_is_coarse_and_legacy_wire_stays_unchanged() {
    let legacy = EditorState::new(CreasePattern::empty())
        .export_history_v1(ProjectId::new())
        .expect("legacy empty history");
    let expected = json!({
        "schema_version": 1,
        "project_id": legacy.project_id(),
        "history_entry_limit": 128,
        "undo_stack": [],
        "redo_stack": [],
    });
    assert_eq!(
        serde_json::to_value(&legacy).expect("legacy JSON"),
        expected
    );
    let project_id_json = serde_json::to_string(&legacy.project_id()).expect("project ID JSON");
    let expected_bytes = format!(
        "{{\"schema_version\":1,\"project_id\":{project_id_json},\"history_entry_limit\":128,\"undo_stack\":[],\"redo_stack\":[]}}"
    )
    .into_bytes();
    assert_eq!(
        serde_json::to_vec(&legacy).expect("legacy bytes"),
        expected_bytes
    );

    let mut fixture = fixture();
    let binding = binding(
        &fixture,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut fixture, binding);
    let json = serde_json::to_value(
        fixture
            .editor
            .export_history_v1(fixture.project_id)
            .expect("marked history"),
    )
    .expect("history JSON");
    let mark = &json["undo_stack"][0]["speculative_unproven_fold_v1"];
    fn assert_no_fine_geometry_keys(value: &serde_json::Value, path: &str) {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, child) in fields {
                    for forbidden in ["vertex", "edge", "face", "coordinate", "shape"] {
                        assert!(
                            !key.contains(forbidden),
                            "{forbidden} key leaked into mark at {path}.{key}"
                        );
                    }
                    assert_no_fine_geometry_keys(child, &format!("{path}.{key}"));
                }
            }
            serde_json::Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    assert_no_fine_geometry_keys(child, &format!("{path}[{index}]"));
                }
            }
            _ => {}
        }
    }
    // Inspect schema keys rather than opaque identity/fingerprint values:
    // canonical UUIDs and SHA-256 strings may legitimately contain an English
    // substring such as "face" by chance.
    assert_no_fine_geometry_keys(mark, "speculative_unproven_fold_v1");
    assert_eq!(
        mark.as_object()
            .expect("mark object")
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        ["binding", "proof_status"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert_eq!(
        mark["binding"]
            .as_object()
            .expect("binding object")
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>(),
        [
            "approximate_blocking_observation",
            "paper_thickness_bits_be",
            "pose_generation",
            "project_id",
            "project_instance_id",
            "request_generation_id",
            "source_geometry_fingerprint_sha256",
            "source_revision",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
}

#[test]
fn inverse_bit_exact_reauthentication_remains_active_with_a_mark() {
    let mut fixture = fixture();
    fixture
        .editor
        .execute(
            0,
            Command::RenameLayer {
                layer: DEFAULT_PROJECT_LAYER_ID,
                name: "Authenticated source layer".to_owned(),
            },
        )
        .expect("anchor source history");
    let binding = binding(
        &fixture,
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    );
    apply_marked(&mut fixture, binding);
    let mut json = serde_json::to_value(
        fixture
            .editor
            .export_history_v1(fixture.project_id)
            .expect("marked history"),
    )
    .expect("history JSON");
    json["undo_stack"][1]["inverse"]["project_layers"]["layers"][0]["name"] =
        Value::String("Tampered but valid".to_owned());
    let decoded = serde_json::from_value::<EditorHistoryV1>(json).expect("well-shaped history");
    assert!(matches!(
        reopen(&fixture.editor, decoded),
        Err(EditorHistoryErrorV1::InverseMismatch)
    ));
}
