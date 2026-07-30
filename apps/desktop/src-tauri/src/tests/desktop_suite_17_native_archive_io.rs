#[test]
fn native_save_as_writes_a_loadable_file_and_preserves_editor_history() {
    let directory = TestDirectory::new();
    let selected_path = directory.join("折り紙設計.backup");
    let expected_path = directory.join("折り紙設計.ori2");
    let mut project = unsaved_project_with_redo_history("First project");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let document = project.document();
    let persisted_document = project
        .project_archive()
        .expect("serializable project")
        .document;
    let can_undo = project.editor.can_undo();
    let can_redo = project.editor.can_redo();

    let response = save_project_as_selected_path(
        &mut project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        selected_path,
    )
    .expect("save project under a selected path");

    assert!(!response.canceled);
    assert_eq!(
        project.current_path.as_deref(),
        Some(expected_path.as_path())
    );
    assert_eq!(project.saved_revision, Some(expected_revision));
    assert_eq!(project.saved_document.as_ref(), Some(&document));
    assert!(!project.is_dirty());
    assert_eq!(project.editor.revision(), expected_revision);
    assert_eq!(project.editor.can_undo(), can_undo);
    assert_eq!(project.editor.can_redo(), can_redo);
    assert_eq!(
        load_document_from_path(&expected_path).unwrap(),
        persisted_document
    );
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
}

#[test]
fn native_save_then_reopen_restores_limit_and_both_history_stacks_in_order() {
    let directory = TestDirectory::new();
    let path = directory.join("history-roundtrip.ori2");
    let (mut source, first, second) =
        unsaved_project_with_undo_and_redo_history("History roundtrip");
    let source_project_id = source.project_id;
    let saved_document = source.document();
    let expected_history = source
        .editor
        .export_history_v1(source_project_id)
        .expect("export source history");

    save_project_to_path(&mut source, path.clone()).expect("save history archive");
    assert_eq!(source.editor.history_entry_limit(), 17);
    assert!(source.editor.can_undo());
    assert!(source.editor.can_redo());
    assert_eq!(
        source
            .editor
            .export_history_v1(source_project_id)
            .expect("history remains usable after save"),
        expected_history
    );

    let mut reopened = ProjectState::new(CreasePattern::empty());
    let replaced_instance_id = reopened.instance_id;
    let replaced_project_id = reopened.project_id;
    let loaded = load_project_file(path.clone()).expect("load saved history archive");
    apply_loaded_project_file(
        &mut reopened,
        replaced_instance_id,
        replaced_project_id,
        0,
        loaded,
    )
    .expect("apply saved history archive");

    assert_eq!(reopened.project_id, source_project_id);
    assert_ne!(reopened.instance_id, replaced_instance_id);
    assert_eq!(reopened.current_path.as_deref(), Some(path.as_path()));
    assert_eq!(reopened.saved_revision, Some(0));
    assert_eq!(reopened.saved_document.as_ref(), Some(&saved_document));
    assert!(!reopened.is_dirty());
    assert_eq!(reopened.editor.revision(), 0);
    assert_eq!(reopened.editor.history_entry_limit(), 17);
    assert!(reopened.editor.can_undo());
    assert!(reopened.editor.can_redo());
    assert!(reopened.editor.current_applied_pose().is_none());
    assert_eq!(
        reopened
            .editor
            .export_history_v1(source_project_id)
            .expect("re-export reopened history"),
        expected_history
    );

    reopened.editor.redo(0).expect("redo second command first");
    assert_eq!(
        reopened
            .editor
            .pattern()
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );
    reopened.editor.undo(1).expect("undo second command");
    assert_eq!(reopened.document(), saved_document);
    reopened.editor.undo(2).expect("undo first command");
    assert!(reopened.editor.pattern().vertices.is_empty());
    reopened.editor.redo(3).expect("redo first command first");
    assert_eq!(reopened.editor.pattern().vertices[0].id, first);
    reopened.editor.redo(4).expect("redo second command second");
    assert_eq!(
        reopened
            .editor
            .pattern()
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );
}

#[test]
fn native_open_legacy_two_entry_archive_uses_default_empty_history() {
    let directory = TestDirectory::new();
    let path = directory.join("legacy-two-entry.ori2");
    let document = file_document("Legacy project", 23.0);
    fs::write(
        &path,
        write_project_ori2(&document).expect("write legacy two-entry archive"),
    )
    .expect("persist legacy archive");

    let mut reopened = ProjectState::new(CreasePattern::empty());
    let loaded = load_project_file(path.clone()).expect("load legacy archive");
    let expected_instance_id = reopened.instance_id;
    let expected_project_id = reopened.project_id;
    apply_loaded_project_file(
        &mut reopened,
        expected_instance_id,
        expected_project_id,
        0,
        loaded,
    )
    .expect("apply legacy archive");

    let mut reopened_document = reopened.document();
    assert!(
        reopened_document.thumbnail_svg.is_some(),
        "legacy archives must gain a canonical thumbnail when projected"
    );
    reopened_document.thumbnail_svg = document.thumbnail_svg.clone();
    assert_eq!(reopened_document, document);
    assert_eq!(reopened.editor.revision(), 0);
    assert_eq!(reopened.editor.history_entry_limit(), 128);
    assert!(!reopened.editor.can_undo());
    assert!(!reopened.editor.can_redo());
    assert_eq!(
        reopened
            .project_archive()
            .expect("export canonical legacy state")
            .editor_history,
        None
    );
    assert!(!reopened.is_dirty());
}

#[test]
fn native_save_overwrites_atomically_and_keeps_undo_redo_history() {
    let directory = TestDirectory::new();
    let path = directory.join("overwrite.ori2");
    fs::write(&path, b"pre-existing invalid project").expect("write overwrite sentinel");
    let mut project = unsaved_project_with_redo_history("Overwrite project");

    save_project_to_path(&mut project, path.clone()).expect("replace existing file");
    let first_bytes = fs::read(&path).expect("read first native save");
    let first_persisted_document = project
        .project_archive()
        .expect("serializable project")
        .document;
    assert_ne!(first_bytes, b"pre-existing invalid project");
    assert_eq!(
        load_document_from_path(&path).unwrap(),
        first_persisted_document
    );
    assert!(project.editor.can_redo());

    let revision_before_redo = project.editor.revision();
    project
        .editor
        .redo(revision_before_redo)
        .expect("restore the saved redo command");
    assert!(project.is_dirty());
    let second_persisted_document = project
        .project_archive()
        .expect("serializable edited project")
        .document;
    let revision_before_save = project.editor.revision();
    let can_undo = project.editor.can_undo();
    let can_redo = project.editor.can_redo();

    save_project_to_path(&mut project, path.clone()).expect("overwrite with edited project");
    let second_bytes = fs::read(&path).expect("read overwritten native save");
    assert_ne!(second_bytes, first_bytes);
    assert_eq!(
        load_document_from_path(&path).unwrap(),
        second_persisted_document
    );
    assert_eq!(project.editor.revision(), revision_before_save);
    assert_eq!(project.editor.can_undo(), can_undo);
    assert_eq!(project.editor.can_redo(), can_redo);
    assert!(!project.is_dirty());
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
}

#[cfg(target_os = "windows")]
#[test]
fn windows_staged_save_denies_concurrent_writers_and_cleans_up() {
    let directory = TestDirectory::new();
    let path = directory.join("writer-sharing.ori2");
    let staged = create_staged_file(&path).expect("create protected staged file");

    let writer_error = OpenOptions::new()
        .write(true)
        .open(&staged.path)
        .expect_err("a concurrent writer must be denied while staging");
    let rename_error = fs::rename(&staged.path, directory.join("swapped-stage"))
        .expect_err("a concurrent rename must be denied while staging");

    assert_eq!(writer_error.raw_os_error(), Some(32));
    assert_eq!(rename_error.raw_os_error(), Some(32));
    drop(staged);
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn native_save_overwrite_preserves_unix_file_mode() {
    use std::os::unix::fs::PermissionsExt;

    let directory = TestDirectory::new();
    let path = directory.join("mode-preservation.ori2");
    fs::write(&path, b"pre-existing invalid project").expect("write mode fixture");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("set fixture mode");
    let mut project = unsaved_project_with_redo_history("Mode preservation");

    save_project_to_path(&mut project, path.clone()).expect("overwrite mode fixture");

    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(load_document_from_path(&path).unwrap(), project.document());
}

#[cfg(unix)]
#[test]
fn unix_directory_sync_failure_is_only_reported_before_publish() {
    let directory = TestDirectory::new();
    let path = directory.join("directory-sync.ori2");
    let document = file_document("Directory sync", 42.0);
    let archive = Ori2ProjectArchive::document_only(document.clone());
    let bytes = write_project_ori2(&document).unwrap();

    fs::write(&path, b"keep before failed pre-publish sync").unwrap();
    let mut staged = prepare_staged_file(&path, &archive, &bytes).unwrap();
    let error = commit_unix_staged_project_file(
        &mut staged,
        &path,
        save_path::ExistingDestinationPolicy::ReplaceConfirmed,
        || Err(std::io::Error::other("injected pre-publish sync failure")),
    )
    .expect_err("a pre-publish directory sync failure must abort the commit");
    assert_eq!(error.kind(), std::io::ErrorKind::Other);
    drop(staged);
    assert_eq!(
        fs::read(&path).unwrap(),
        b"keep before failed pre-publish sync"
    );
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);

    let mut staged = prepare_staged_file(&path, &archive, &bytes).unwrap();
    let mut sync_calls = 0_u8;
    commit_unix_staged_project_file(
        &mut staged,
        &path,
        save_path::ExistingDestinationPolicy::ReplaceConfirmed,
        || {
            sync_calls += 1;
            if sync_calls == 1 {
                Ok(())
            } else {
                Err(std::io::Error::other("injected post-publish sync failure"))
            }
        },
    )
    .expect("a post-publish durability failure must not report an ordinary save failure");
    drop(staged);

    assert_eq!(sync_calls, 2);
    assert_eq!(load_document_from_path(&path).unwrap(), document);
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
}

#[test]
fn native_open_replaces_the_project_only_after_loading_and_validation() {
    let directory = TestDirectory::new();
    let path = directory.join("opened.ori2");
    let mut document = file_document("Opened project", 42.0);
    document.paper.cutting_allowed = true;
    persist_document(&path, &document).expect("write open fixture");

    let mut project = unsaved_project_with_redo_history("Replaced project");
    let expected_instance_id = project.instance_id;
    let replaced_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let loaded = load_project_file(path.clone()).expect("load native project");
    let response = apply_loaded_project_file(
        &mut project,
        expected_instance_id,
        replaced_project_id,
        expected_revision,
        loaded,
    )
    .expect("apply validated native project");

    assert!(!response.canceled);
    assert_ne!(project.project_id, replaced_project_id);
    let persisted = project.document();
    assert!(persisted.thumbnail_svg.is_some());
    document.thumbnail_svg = persisted.thumbnail_svg.clone();
    assert_eq!(persisted, document);
    assert_eq!(project.current_path.as_deref(), Some(path.as_path()));
    assert_eq!(project.editor.revision(), 0);
    assert!(!project.editor.can_undo());
    assert!(!project.editor.can_redo());
    assert!(!project.is_dirty());
}

#[test]
fn corrupt_native_open_preserves_project_state_and_history() {
    let directory = TestDirectory::new();
    let secret_name = "private-client-corrupt.ori2";
    let path = directory.join(secret_name);
    let private_payload = b"not an ORIGAMI2 archive: SECRET_PROJECT_CONTENT";
    fs::write(&path, private_payload).expect("write corrupt fixture");
    let project = unsaved_project_with_redo_history("Unaffected project");
    let before = project_state_signature(&project);

    let error = load_project_file(path.clone()).expect_err("corrupt project must fail validation");

    assert_eq!(error, PROJECT_FILE_INVALID_MESSAGE);
    assert!(!error.contains(secret_name));
    assert!(!error.contains("SECRET_PROJECT_CONTENT"));
    assert!(!error.contains(&directory.path.to_string_lossy().into_owned()));
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn corrupt_native_history_open_preserves_every_existing_project_field() {
    let directory = TestDirectory::new();
    let secret_name = "private-client-history-corrupt.ori2";
    let path = directory.join(secret_name);
    let (source, _, _) = unsaved_project_with_undo_and_redo_history("History corruption source");
    persist_project_archive(
        &path,
        &source.project_archive().expect("export source archive"),
    )
    .expect("write valid archive before targeted corruption");
    let corrupt_bytes =
        corrupt_editor_history_payload(fs::read(&path).expect("read valid history archive"));
    fs::write(&path, corrupt_bytes).expect("corrupt only the compressed history payload");

    let (project, _, _) = unsaved_project_with_undo_and_redo_history("Unaffected active project");
    let before = project_state_signature(&project);
    let error = load_project_file(path).expect_err("corrupt editor history must reject the open");

    assert_eq!(error, PROJECT_FILE_INVALID_MESSAGE);
    assert!(!error.contains(secret_name));
    assert!(!error.contains(&directory.path.to_string_lossy().into_owned()));
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn save_rejects_an_invalid_instruction_pose_at_a_reachable_history_endpoint() {
    let directory = TestDirectory::new();
    let path = directory.join("must-not-save-reachable-pose.ori2");
    let mut project = project_with_reachable_invalid_instruction_pose("Unsafe history endpoint");
    let before = project_state_signature(&project);

    let error = save_project_to_path(&mut project, path.clone())
        .expect_err("save must validate every reachable history endpoint");

    assert_eq!(error, PROJECT_SERIALIZATION_FAILED_MESSAGE);
    assert_eq!(project_state_signature(&project), before);
    assert!(!path.exists());
}

#[test]
fn save_rejects_an_invalid_instruction_pose_at_a_redo_endpoint() {
    let directory = TestDirectory::new();
    let path = directory.join("must-not-save-redo-pose.ori2");
    let mut project = project_with_redo_reachable_invalid_instruction_pose("Unsafe Redo endpoint");
    let before = project_state_signature(&project);

    let error = save_project_to_path(&mut project, path.clone())
        .expect_err("save must validate every reachable Redo endpoint");

    assert_eq!(error, PROJECT_SERIALIZATION_FAILED_MESSAGE);
    assert_eq!(project_state_signature(&project), before);
    assert!(!path.exists());
}

#[test]
fn native_open_rejects_reachable_invalid_pose_history_without_mutating_current_state() {
    let directory = TestDirectory::new();
    let secret_name = "private-reachable-pose-history.ori2";
    let path = directory.join(secret_name);
    let source =
        project_with_reachable_invalid_instruction_pose("External unsafe history endpoint");
    let external_archive = Ori2ProjectArchive {
        layer_evidence: None,
        document: source.document(),
        editor_history: Some(
            source
                .editor
                .export_history_v1(source.project_id)
                .expect("export external history fixture"),
        ),
    };
    fs::write(
        &path,
        write_project_archive_ori2(&external_archive)
            .expect("the format boundary accepts replay-consistent external history"),
    )
    .expect("write external history fixture");

    let (active, _, _) = unsaved_project_with_undo_and_redo_history("Unaffected active project");
    let before = project_state_signature(&active);
    let error = load_project_file(path).expect_err("semantic history endpoint must reject open");

    assert_eq!(error, PROJECT_FILE_INVALID_MESSAGE);
    assert!(!error.contains(secret_name));
    assert!(!error.contains("instruction"));
    assert_eq!(project_state_signature(&active), before);
}

#[test]
fn internal_archive_restore_rejects_a_history_bound_to_another_project() {
    let (source, _, _) = unsaved_project_with_undo_and_redo_history("Bound history");
    let mut archive = source.project_archive().expect("export bound history");
    archive.document.project_id = ProjectId::new();

    assert!(restore_archive_editor(&archive).is_err());
}

#[test]
fn native_open_file_failures_use_fixed_path_free_categories() {
    let directory = TestDirectory::new();
    let secret_name = "private-client-missing.ori2";
    let missing_path = directory.join(secret_name);

    let missing_error =
        load_project_file(missing_path).expect_err("missing project must be rejected");
    assert_eq!(missing_error, PROJECT_FILE_OPEN_FAILED_MESSAGE);
    assert!(!missing_error.contains(secret_name));
    assert!(!missing_error.contains(&directory.path.to_string_lossy().into_owned()));
    assert!(!missing_error.to_ascii_lowercase().contains("os error"));

    let oversized_name = "private-client-oversized.ori2";
    let oversized_path = directory.join(oversized_name);
    File::create(&oversized_path)
        .expect("create oversized project fixture")
        .set_len(Ori2Limits::default().max_archive_size + 1)
        .expect("make sparse oversized project fixture");

    let oversized_error =
        load_project_file(oversized_path).expect_err("oversized project must be rejected");
    assert_eq!(oversized_error, PROJECT_FILE_TOO_LARGE_MESSAGE);
    assert!(!oversized_error.contains(oversized_name));
    assert!(!oversized_error.contains(&(Ori2Limits::default().max_archive_size + 1).to_string()));
    assert!(!oversized_error.contains(&directory.path.to_string_lossy().into_owned()));
}

#[test]
fn native_open_instruction_failure_discards_private_semantic_details() {
    let project = initial_project_state();
    let mut document = project.document();
    let private_title = "SECRET_PRIVATE_INSTRUCTION";
    let private_face = FaceId::new();
    document.instruction_timeline.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: private_title.to_owned(),
        description: String::new(),
        caution: String::new(),
        duration_ms: 1_000,
        visual: Default::default(),
        pose: InstructionPose {
            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
            source_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
            fixed_face: Some(private_face),
            hinge_angles: Vec::new(),
        },
    });
    let bytes = write_project_ori2(&document)
        .expect("syntactically valid project can carry a semantically invalid pose");
    let directory = TestDirectory::new();
    let secret_name = "private-instruction-project.ori2";
    let path = directory.join(secret_name);
    fs::write(&path, bytes).expect("write instruction failure fixture");

    let error = load_project_file(path).expect_err("semantic instruction failure must be rejected");

    assert_eq!(error, PROJECT_INSTRUCTIONS_INVALID_MESSAGE);
    assert!(!error.contains(private_title));
    assert!(!error.contains(&format!("{private_face:?}")));
    assert!(!error.contains(secret_name));
    assert!(!error.contains(&directory.path.to_string_lossy().into_owned()));
}

#[test]
fn stale_native_open_is_rejected_without_replacing_newer_history() {
    let directory = TestDirectory::new();
    let path = directory.join("stale-open.ori2");
    persist_document(&path, &file_document("Stale open", 17.0)).expect("write stale-open fixture");
    let mut project = unsaved_project_with_redo_history("Active project");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let stale_revision = project.editor.revision();
    let loaded = load_project_file(path).expect("prepare native open");
    execute_command(
        &mut project,
        expected_project_id,
        stale_revision,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(8.0, 9.0),
        },
    )
    .expect("edit while the file dialog is open");
    let before_apply = project_state_signature(&project);

    let error = apply_loaded_project_file(
        &mut project,
        expected_instance_id,
        expected_project_id,
        stale_revision,
        loaded,
    )
    .expect_err("stale open must not replace the active project");

    assert_eq!(error, "the project changed while the file dialog was open");
    assert_eq!(project_state_signature(&project), before_apply);
}

#[test]
fn native_file_dialog_results_cannot_land_after_reopening_the_same_document() {
    let directory = TestDirectory::new();
    let current_path = directory.join("same-document.ori2");
    let opened_path = directory.join("other-document.ori2");
    let selected_path = directory.join("must-not-save.ori2");
    let document = file_document("Same document", 21.0);
    persist_document(&current_path, &document).expect("write same-document fixture");
    persist_document(&opened_path, &file_document("Other document", 34.0))
        .expect("write other-document fixture");

    let mut project = ProjectState::from_valid_document(document.clone(), current_path.clone());
    let stale_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let loaded = load_project_file(opened_path).expect("load delayed open result");

    project = ProjectState::from_valid_document(document, current_path);
    assert_eq!(project.project_id, expected_project_id);
    assert_eq!(project.editor.revision(), expected_revision);
    assert_ne!(project.instance_id, stale_instance_id);
    let before = project_state_signature(&project);

    let open_error = apply_loaded_project_file(
        &mut project,
        stale_instance_id,
        expected_project_id,
        expected_revision,
        loaded,
    )
    .expect_err("a delayed open must not replace a reopened project instance");
    assert_eq!(
        open_error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&project), before);

    let save_error = save_project_as_selected_path(
        &mut project,
        stale_instance_id,
        expected_project_id,
        expected_revision,
        selected_path.clone(),
    )
    .expect_err("a delayed save must not target a reopened project instance");
    assert_eq!(
        save_error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&project), before);
    assert!(!selected_path.exists());
}
