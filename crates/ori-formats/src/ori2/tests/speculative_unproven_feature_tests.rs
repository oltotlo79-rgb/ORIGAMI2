use crate::{
    ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1, PROJECT_FOLDER_EDITOR_HISTORY_PATH,
    ProjectFolderError, read_project_folder_v1, write_project_folder_v1,
};

use super::{speculative_unproven_feature_test_support::*, *};

fn assert_ori2_feature_mismatch(bytes: &[u8]) {
    assert!(matches!(
        read_project_archive_ori2(bytes),
        Err(FormatError::RequiredFeaturesMismatch { .. })
    ));
}

fn assert_folder_feature_mismatch(entries: &[crate::ProjectFolderEntryV1]) {
    assert!(matches!(
        read_project_folder_v1(entries),
        Err(ProjectFolderError::RequiredFeaturesMismatch { .. })
    ));
}

#[test]
fn undo_and_applied_base_marks_round_trip_through_both_formats() {
    assert_eq!(
        ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1,
        "speculative_unproven_fold_v1"
    );
    for applied_base in [false, true] {
        let archive = marked_archive(applied_base);
        let expected = if applied_base {
            vec![
                ORI2_FEATURE_INSTRUCTION_TIMELINE_V1.to_owned(),
                ORI2_FEATURE_LAYERS_V1.to_owned(),
                ORI2_FEATURE_EDITOR_HISTORY_V1.to_owned(),
                ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1.to_owned(),
            ]
        } else {
            vec![
                ORI2_FEATURE_INSTRUCTION_TIMELINE_V1.to_owned(),
                ORI2_FEATURE_EDITOR_HISTORY_V1.to_owned(),
                ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1.to_owned(),
            ]
        };

        let ori2 = write_project_archive_ori2(&archive).expect("write marked ori2");
        assert_eq!(manifest_from_archive(&ori2).required_features, expected);
        let ori2_read = read_project_archive_ori2(&ori2).expect("read marked ori2");
        assert_eq!(ori2_read, archive);
        assert_eq!(
            write_project_archive_ori2(&ori2_read).expect("resave marked ori2"),
            ori2
        );
        assert!(
            ori2_read
                .editor_history
                .as_ref()
                .expect("ori2 history")
                .requires_speculative_unproven_fold_feature_v1()
        );

        let folder = write_project_folder_v1(&archive).expect("write marked folder");
        assert_eq!(
            folder_manifest(folder.entries()).required_features,
            expected
        );
        let folder_read = read_project_folder_v1(folder.entries()).expect("read marked folder");
        assert_eq!(folder_read.archive(), &archive);
        let recovered =
            write_project_folder_v1(folder_read.archive()).expect("resave recovered folder");
        assert_eq!(recovered.entries(), folder.entries());
        assert_eq!(
            read_project_folder_v1(recovered.entries())
                .expect("reread recovered folder")
                .archive(),
            &archive
        );

        if applied_base {
            let history_json = folder
                .entries()
                .iter()
                .find(|entry| entry.path == PROJECT_FOLDER_EDITOR_HISTORY_PATH)
                .expect("folder history");
            assert!(
                String::from_utf8_lossy(&history_json.bytes)
                    .contains("speculative_unproven_applied_base_v1")
            );
        }
    }
}

#[test]
fn ori2_feature_and_history_tampering_is_rejected_fail_closed() {
    let original = write_project_archive_ori2(&marked_archive(false)).expect("marked ori2");

    let missing = rewrite_ori2_manifest(&original, |manifest| {
        manifest
            .required_features
            .retain(|feature| feature != ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1);
    });
    assert_ori2_feature_mismatch(&missing);

    let duplicate = rewrite_ori2_manifest(&original, |manifest| {
        manifest
            .required_features
            .push(ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1.to_owned());
    });
    assert_ori2_feature_mismatch(&duplicate);

    let reordered = rewrite_ori2_manifest(&original, |manifest| {
        let last = manifest.required_features.len() - 1;
        manifest.required_features.swap(last - 1, last);
    });
    assert_ori2_feature_mismatch(&reordered);

    let unknown = rewrite_ori2_manifest(&original, |manifest| {
        manifest
            .required_features
            .push("future_speculative_fold_v9".to_owned());
    });
    assert!(matches!(
        read_project_archive_ori2(&unknown),
        Err(FormatError::UnsupportedRequiredFeatures { features })
            if features == vec!["future_speculative_fold_v9".to_owned()]
    ));

    assert_ori2_feature_mismatch(&ori2_without_history_mark(&original));
    assert!(matches!(
        read_project_archive_ori2(&ori2_without_history_entry(&original)),
        Err(FormatError::MissingEntry {
            path: ORI2_EDITOR_HISTORY_PATH
        })
    ));
}

#[test]
fn expanded_folder_feature_and_history_tampering_is_rejected_fail_closed() {
    let original = write_project_folder_v1(&marked_archive(false)).expect("marked folder");

    let missing = rewrite_folder_manifest(original.entries(), |manifest| {
        manifest
            .required_features
            .retain(|feature| feature != ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1);
    });
    assert_folder_feature_mismatch(&missing);

    let duplicate = rewrite_folder_manifest(original.entries(), |manifest| {
        manifest
            .required_features
            .push(ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1.to_owned());
    });
    assert_folder_feature_mismatch(&duplicate);

    let reordered = rewrite_folder_manifest(original.entries(), |manifest| {
        let last = manifest.required_features.len() - 1;
        manifest.required_features.swap(last - 1, last);
    });
    assert_folder_feature_mismatch(&reordered);

    let unknown = rewrite_folder_manifest(original.entries(), |manifest| {
        manifest
            .required_features
            .push("future_speculative_fold_v9".to_owned());
    });
    assert!(matches!(
        read_project_folder_v1(&unknown),
        Err(ProjectFolderError::UnsupportedRequiredFeatures { features })
            if features == vec!["future_speculative_fold_v9".to_owned()]
    ));

    assert_folder_feature_mismatch(&folder_without_history_mark(original.entries()));
    let mut missing_history = original.entries().to_vec();
    missing_history.retain(|entry| entry.path != PROJECT_FOLDER_EDITOR_HISTORY_PATH);
    assert!(matches!(
        read_project_folder_v1(&missing_history),
        Err(ProjectFolderError::MissingEntry {
            path: PROJECT_FOLDER_EDITOR_HISTORY_PATH
        } | ProjectFolderError::NonCanonicalEntryOrder { .. })
    ));
}

#[test]
fn feature_only_forgery_is_rejected_and_markless_history_stays_legacy_exact() {
    let document = sample_document();
    let document_only =
        write_project_archive_ori2(&Ori2ProjectArchive::document_only(document.clone()))
            .expect("document-only ori2");
    let forged = rewrite_ori2_manifest(&document_only, |manifest| {
        manifest
            .required_features
            .push(ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1.to_owned());
    });
    assert_ori2_feature_mismatch(&forged);

    let document_folder =
        write_project_folder_v1(&Ori2ProjectArchive::document_only(document.clone()))
            .expect("document-only folder");
    let forged_folder = rewrite_folder_manifest(document_folder.entries(), |manifest| {
        manifest
            .required_features
            .push(ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1.to_owned());
    });
    assert_folder_feature_mismatch(&forged_folder);

    let history = empty_editor_history(document.project_id, 17);
    assert!(!history.requires_speculative_unproven_fold_feature_v1());
    let legacy_archive = Ori2ProjectArchive {
        document: document.clone(),
        editor_history: Some(history),
        layer_evidence: None,
    };
    let legacy_ori2 = write_project_archive_ori2(&legacy_archive).expect("legacy history ori2");
    assert_eq!(
        manifest_from_archive(&legacy_ori2).required_features,
        vec![ORI2_FEATURE_EDITOR_HISTORY_V1.to_owned()]
    );
    let entries = archive_entries(&legacy_ori2);
    let history_bytes = &entries
        .iter()
        .find(|(path, _)| path == ORI2_EDITOR_HISTORY_PATH)
        .expect("legacy history entry")
        .1;
    let envelope: serde_json::Value =
        serde_json::from_slice(history_bytes).expect("legacy history JSON");
    assert_eq!(
        envelope["history"],
        serde_json::json!({
            "schema_version": EDITOR_HISTORY_SCHEMA_VERSION_V1,
            "project_id": document.project_id,
            "history_entry_limit": 17,
            "undo_stack": [],
            "redo_stack": [],
        })
    );
    assert!(
        !String::from_utf8_lossy(history_bytes).contains(ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1)
    );

    let legacy_folder = write_project_folder_v1(&legacy_archive).expect("legacy history folder");
    assert_eq!(
        folder_manifest(legacy_folder.entries()).required_features,
        vec![ORI2_FEATURE_EDITOR_HISTORY_V1.to_owned()]
    );
    assert_eq!(
        write_project_json(
            &read_project_folder_v1(legacy_folder.entries())
                .expect("legacy folder read")
                .archive()
                .document
        )
        .expect("restored project JSON"),
        write_project_json(&document).expect("source project JSON")
    );
}

#[test]
fn legacy_known_feature_allowlist_rejects_marked_archive() {
    let bytes = write_project_archive_ori2(&marked_archive(false)).expect("marked ori2");
    let manifest = manifest_from_archive(&bytes);
    let legacy_result =
        super::super::validate_required_features_with_allowlist_v1(&manifest, |feature| {
            matches!(
                feature,
                ORI2_FEATURE_INSTRUCTION_TIMELINE_V1
                    | ORI2_FEATURE_DECLARATIVE_INSTRUCTION_STEPS_V1
                    | ORI2_FEATURE_NUMERIC_EXPRESSIONS_V1
                    | ORI2_FEATURE_GEOMETRIC_CONSTRAINTS_V1
                    | ORI2_FEATURE_LAYERS_V1
                    | ORI2_FEATURE_REFERENCE_MODEL_ASSETS_V1
                    | ORI2_FEATURE_EDITOR_HISTORY_V1
                    | ORI2_FEATURE_LAYER_EVIDENCE_V1
            )
        });
    assert!(matches!(
        legacy_result,
        Err(FormatError::UnsupportedRequiredFeatures { features })
            if features == vec![ORI2_FEATURE_SPECULATIVE_UNPROVEN_FOLD_V1.to_owned()]
    ));
}
