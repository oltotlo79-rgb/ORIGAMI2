use ori_core::{
    SpeculativeApproximateBlockingObservationV1, SpeculativeUnprovenFoldBindingV1,
    SpeculativeUnprovenFoldProofOutcomeV1, SpeculativeUnprovenFoldStatusCountsV1,
    SpeculativeUnprovenFoldSummaryV1,
};

use super::*;

static NEXT_DIRECTORY_ID: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let id = NEXT_DIRECTORY_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "origami2-speculative-native-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("test directory");
        Self(path)
    }

    fn join(&self, name: impl AsRef<Path>) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn marked_project_v1() -> (ProjectState, SpeculativeUnprovenFoldBindingV1) {
    let app_state = crate::stacked_fold_read::tests::prepare_applied_speculative_project_v1();
    let AppState(project, _, _, _) = app_state;
    let project = project
        .into_inner()
        .expect("production speculative project");
    let history = project
        .editor
        .export_history_v1(project.project_id)
        .expect("marked history");
    let history = serde_json::to_value(history).expect("history wire");
    let binding = &history["undo_stack"]
        .as_array()
        .and_then(|entries| entries.last())
        .expect("one applied speculative entry")["speculative_unproven_fold_v1"]["binding"];
    let parse_id = |field: &str| {
        serde_json::from_value::<ProjectId>(binding[field].clone())
            .unwrap_or_else(|_| panic!("valid {field}"))
    };
    let thickness_bytes: [u8; 8] =
        serde_json::from_value(binding["paper_thickness_bits_be"].clone())
            .expect("exact thickness bytes");
    let binding = SpeculativeUnprovenFoldBindingV1::new(
        parse_id("project_instance_id"),
        parse_id("project_id"),
        binding["source_revision"]
            .as_u64()
            .expect("source revision"),
        binding["source_geometry_fingerprint_sha256"]
            .as_str()
            .expect("source fingerprint")
            .to_owned(),
        binding["pose_generation"]
            .as_u64()
            .expect("pose generation"),
        parse_id("request_generation_id"),
        f64::from_bits(u64::from_be_bytes(thickness_bytes)),
        SpeculativeApproximateBlockingObservationV1::no_blocking_sample_observed(),
    )
    .expect("restored non-authority binding");
    (project, binding)
}

#[test]
fn snapshot_summary_is_fresh_exact_and_coarse() {
    let (project, binding) = marked_project_v1();
    let first = snapshot(&project);
    assert_eq!(first.speculative_unproven_folds.applied.awaiting_proof, 1);
    let app = AppState::new(project);
    let before = {
        let project = lock_project(&app).expect("project");
        (
            project.editor.revision(),
            project.editor.pattern().clone(),
            project.editor.instruction_timeline().clone(),
        )
    };
    let dto = stacked_fold_transaction::resolve_speculative_unproven_fold_native_v1(
        &app,
        &binding,
        SpeculativeUnprovenFoldProofOutcomeV1::Blocked,
    )
    .expect("native proof result");
    assert_eq!(
        serde_json::to_value(dto).expect("resolution DTO")["outcome"],
        serde_json::json!("blocked")
    );

    let project = lock_project(&app).expect("project");
    assert_eq!(project.editor.revision(), before.0);
    assert_eq!(project.editor.pattern(), &before.1);
    assert_eq!(project.editor.instruction_timeline(), &before.2);
    assert!(project.is_dirty());
    let second = serde_json::to_value(snapshot(&project)).expect("snapshot");
    let summary = &second["speculativeUnprovenFolds"];
    assert_eq!(
        summary,
        &serde_json::json!({
            "applied": {
                "awaitingProof": 0,
                "proofBlocked": 1,
                "unknownEvidenceInsufficient": 0,
                "unknownResourceLimit": 0,
                "unknownCancelled": 0,
                "unknownDeadlineReached": 0
            },
            "unappliedRedo": {
                "awaitingProof": 0,
                "proofBlocked": 0,
                "unknownEvidenceInsufficient": 0,
                "unknownResourceLimit": 0,
                "unknownCancelled": 0,
                "unknownDeadlineReached": 0
            }
        })
    );
    let coarse = summary.to_string();
    for identity in [project.instance_id, project.project_id] {
        let wire_identity = serde_json::to_value(identity)
            .expect("identity JSON")
            .as_str()
            .expect("identity string")
            .to_owned();
        assert!(!coarse.contains(&wire_identity));
    }
    for forbidden in [
        "binding",
        "geometry",
        "fingerprint",
        "vertex",
        "edge",
        "face",
    ] {
        assert!(!coarse.contains(forbidden), "leaked {forbidden}");
    }
}

#[test]
fn every_coarse_status_category_has_an_exact_wire_counter() {
    let summary = SpeculativeUnprovenFoldSummaryDtoV1::from(SpeculativeUnprovenFoldSummaryV1 {
        applied: SpeculativeUnprovenFoldStatusCountsV1 {
            awaiting_proof: 1,
            proof_blocked: 2,
            unknown_evidence_insufficient: 3,
            unknown_resource_limit: 4,
            unknown_cancelled: 5,
            unknown_deadline_reached: 6,
        },
        unapplied_redo: SpeculativeUnprovenFoldStatusCountsV1 {
            awaiting_proof: 7,
            proof_blocked: 8,
            unknown_evidence_insufficient: 9,
            unknown_resource_limit: 10,
            unknown_cancelled: 11,
            unknown_deadline_reached: 12,
        },
    });
    let value = serde_json::to_value(summary).expect("summary");
    assert_eq!(value["applied"]["unknownDeadlineReached"], 6);
    assert_eq!(value["unappliedRedo"]["awaitingProof"], 7);
    assert_eq!(value["unappliedRedo"]["unknownCancelled"], 11);
    assert_eq!(value["unappliedRedo"].as_object().expect("counts").len(), 6);
}

#[test]
fn dirty_baseline_save_reload_and_undo_redo_track_status_only_changes() {
    let directory = TestDirectory::new();
    let path = directory.join("status-history.ori2");
    let (mut project, binding) = marked_project_v1();
    assert!(project.is_dirty());
    project
        .editor
        .export_history_v1(project.project_id)
        .expect("export marked history");

    save_project_to_path(&mut project, path.clone()).expect("save awaiting state");
    assert!(!project.is_dirty());
    project
        .editor
        .resolve_speculative_unproven_fold_v1(
            &binding,
            SpeculativeUnprovenFoldProofOutcomeV1::Blocked,
        )
        .expect("record proof failure");
    assert!(project.is_dirty(), "status-only changes must be dirty");
    assert_eq!(
        snapshot(&project)
            .speculative_unproven_folds
            .applied
            .proof_blocked,
        1
    );

    save_project_to_path(&mut project, path.clone()).expect("save blocked state");
    assert!(!project.is_dirty());
    let mut reopened = load_project_file(path)
        .expect("load blocked state")
        .replacement;
    assert!(!reopened.is_dirty());
    assert_eq!(
        snapshot(&reopened)
            .speculative_unproven_folds
            .applied
            .proof_blocked,
        1
    );

    let revision = reopened.editor.revision();
    reopened.editor.undo(revision).expect("undo marked Apply");
    let undone = snapshot(&reopened);
    assert_eq!(undone.speculative_unproven_folds.applied.proof_blocked, 0);
    assert_eq!(
        undone
            .speculative_unproven_folds
            .unapplied_redo
            .proof_blocked,
        1
    );
    assert!(reopened.is_dirty());

    let revision = reopened.editor.revision();
    reopened.editor.redo(revision).expect("redo marked Apply");
    let redone = snapshot(&reopened);
    assert_eq!(redone.speculative_unproven_folds.applied.proof_blocked, 1);
    assert_eq!(
        redone
            .speculative_unproven_folds
            .unapplied_redo
            .proof_blocked,
        0
    );
    assert!(!reopened.is_dirty());
}

#[test]
fn cancelled_or_failed_save_does_not_advance_the_status_baseline() {
    let directory = TestDirectory::new();
    let (project, _) = marked_project_v1();
    let expected_baseline = project.saved_speculative_unproven_state.clone();
    let app = AppState::new(project);
    let response = canceled_file_response(&app).expect("cancelled response");
    assert!(response.canceled);
    {
        let project = lock_project(&app).expect("project");
        assert_eq!(project.saved_speculative_unproven_state, expected_baseline);
        assert!(project.is_dirty());
    }

    let mut project = lock_project(&app).expect("project");
    let missing_parent = directory.join("missing").join("failed.ori2");
    assert!(save_project_to_path(&mut project, missing_parent).is_err());
    assert_eq!(project.saved_speculative_unproven_state, expected_baseline);
    assert!(project.is_dirty());
}
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
