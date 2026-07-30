#[test]
fn native_save_failure_preserves_state_history_and_existing_target() {
    let directory = TestDirectory::new();
    let occupied_path = directory.join("occupied.ori2");
    fs::create_dir(&occupied_path).expect("create an unreplaceable save target");
    let sentinel = occupied_path.join("keep.txt");
    fs::write(&sentinel, b"keep this directory").expect("write save-failure sentinel");
    let mut project = unsaved_project_with_redo_history("Failed save");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let before = project_state_signature(&project);

    let error = save_project_as_selected_path(
        &mut project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        occupied_path.clone(),
    )
    .expect_err("a directory cannot be replaced by a project file");

    assert_eq!(
        error,
        "プロジェクトを保存先へ安全に確定できなかったため、保存を中止しました。"
    );
    assert!(!error.contains("occupied.ori2"));
    assert!(!error.contains(&directory.path.display().to_string()));
    assert_eq!(project_state_signature(&project), before);
    assert_eq!(fs::read(&sentinel).unwrap(), b"keep this directory");
    assert!(occupied_path.is_dir());
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
}

#[test]
fn stale_native_save_as_is_rejected_before_touching_the_selected_path() {
    let directory = TestDirectory::new();
    let selected_path = directory.join("stale-save");
    let normalized_path = directory.join("stale-save.ori2");
    let mut project = unsaved_project_with_redo_history("Stale save");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let stale_revision = project.editor.revision();
    execute_command(
        &mut project,
        expected_project_id,
        stale_revision,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(99.0, 100.0),
        },
    )
    .expect("edit before stale save-as is applied");
    let before_save = project_state_signature(&project);

    let error = save_project_as_selected_path(
        &mut project,
        expected_instance_id,
        expected_project_id,
        stale_revision,
        selected_path,
    )
    .expect_err("stale save-as must fail");

    assert_eq!(error, "the project changed while the file dialog was open");
    assert_eq!(project_state_signature(&project), before_save);
    assert!(!normalized_path.exists());
}

#[test]
fn native_save_as_cannot_overwrite_an_existing_unconfirmed_corrected_path() {
    let directory = TestDirectory::new();
    let selected_path = directory.join("project.txt");
    let corrected_path = directory.join("project.ori2");
    fs::write(&corrected_path, b"keep existing project").unwrap();
    let mut project = unsaved_project_with_redo_history("Protected project");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let before = project_state_signature(&project);

    let error = save_project_as_selected_path(
        &mut project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        selected_path.clone(),
    )
    .expect_err("an unconfirmed corrected destination must not be overwritten");

    assert!(error.contains("上書き確認"));
    assert_eq!(project_state_signature(&project), before);
    assert_eq!(fs::read(corrected_path).unwrap(), b"keep existing project");
    assert!(!selected_path.exists());
}

#[test]
fn project_save_target_conversion_error_discards_the_raw_path_and_os_error() {
    let raw_error = r"C:\Users\private-work\secret.ori2: injected operating-system detail";

    let error = project_save_target_conversion_error(raw_error);

    assert_eq!(error, "選択された保存先はローカルファイルではありません。");
    assert!(!error.contains("private-work"));
    assert!(!error.contains("operating-system"));
}

#[test]
fn extension_correction_race_cannot_replace_a_new_destination() {
    let directory = TestDirectory::new();
    let selected_path = directory.join("race-target.backup");
    let corrected_path = directory.join("race-target.ori2");
    let destination =
        ensure_ori2_extension(selected_path).expect("preflight an unused corrected path");
    assert_eq!(
        destination.existing_destination_policy(),
        save_path::ExistingDestinationPolicy::RejectExisting
    );

    let protected_bytes = b"created after extension preflight";
    fs::write(&corrected_path, protected_bytes).unwrap();
    let mut project = unsaved_project_with_redo_history("Race-safe project");
    let before = project_state_signature(&project);

    let error = save_project_to_destination(&mut project, destination)
        .expect_err("atomic create-new commit must reject the intervening destination");

    assert!(error.contains("安全に確定"));
    assert!(!error.contains("race-target"));
    assert_eq!(fs::read(&corrected_path).unwrap(), protected_bytes);
    assert_eq!(project_state_signature(&project), before);
    assert!(
        fs::read_dir(&directory.path)
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| !entry
                .file_name()
                .to_string_lossy()
                .starts_with(".origami2-")),
        "a rejected create-new commit must clean its staged file"
    );
}

#[test]
fn correct_extension_keeps_the_dialog_confirmed_overwrite() {
    let directory = TestDirectory::new();
    let path = directory.join("confirmed.ori2");
    fs::write(&path, b"OS-confirmed old bytes").unwrap();
    let mut project = unsaved_project_with_redo_history("Confirmed overwrite");
    let expected_persisted_document = project
        .project_archive()
        .expect("serializable project")
        .document;
    let destination =
        ensure_ori2_extension(path.clone()).expect("accept a dialog-confirmed extension");

    save_project_to_destination(&mut project, destination)
        .expect("replace the dialog-confirmed destination");

    assert_eq!(
        load_document_from_path(&path).unwrap(),
        expected_persisted_document
    );
    assert_eq!(project.current_path.as_deref(), Some(path.as_path()));
}

#[test]
fn save_as_extension_is_normalized_without_changing_valid_case() {
    assert_eq!(
        ensure_ori2_extension(PathBuf::from("crane")).unwrap(),
        PathBuf::from("crane.ori2")
    );
    assert_eq!(
        ensure_ori2_extension(PathBuf::from("crane.json")).unwrap(),
        PathBuf::from("crane.ori2")
    );
    assert_eq!(
        ensure_ori2_extension(PathBuf::from("crane.ORI2")).unwrap(),
        PathBuf::from("crane.ORI2")
    );
}

#[test]
fn relative_save_path_uses_the_current_directory_for_staging_and_sync() {
    assert_eq!(
        containing_directory(Path::new("bird.ori2")),
        Some(Path::new("."))
    );
    assert_eq!(
        containing_directory(Path::new("projects/bird.ori2")),
        Some(Path::new("projects"))
    );
}

#[cfg(target_os = "windows")]
#[test]
fn windows_relative_save_path_publishes_the_verified_project() {
    let directory = TestDirectory::new_relative();
    let path = directory.join("relative.ori2");
    let document = file_document("Relative Windows save", 31.0);
    assert!(path.is_relative());

    persist_document(&path, &document).expect("publish to a relative Windows path");

    assert_eq!(load_document_from_path(&path).unwrap(), document);
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
}

#[test]
fn suggested_name_removes_platform_forbidden_characters() {
    assert_eq!(
        suggested_file_name("  Bird: prototype?  "),
        "Bird_ prototype_.ori2"
    );
    assert_eq!(suggested_file_name("..."), "Untitled.ori2");
}

#[test]
fn generated_container_verification_is_pure_and_checks_identity() {
    let document = ProjectDocument::new("Bird", CreasePattern::empty());
    let archive = Ori2ProjectArchive::document_only(document.clone());
    let bytes = write_project_ori2(&document).expect("generate .ori2");
    verify_generated_ori2(&archive, &bytes).expect("verify generated .ori2");

    let different_document = ProjectDocument::new("Different", CreasePattern::empty());
    let different_archive = Ori2ProjectArchive::document_only(different_document);
    let error = verify_generated_ori2(&different_archive, &bytes)
        .expect_err("a different project must not verify");
    assert_eq!(error, "generated .ori2 data did not round-trip exactly");

    let (history_project, _, _) =
        unsaved_project_with_undo_and_redo_history("History must not disappear");
    let history_archive = history_project
        .project_archive()
        .expect("export nonempty history");
    let document_only_bytes = write_project_ori2(&history_archive.document)
        .expect("write bytes that intentionally omit history");
    let error = verify_generated_ori2(&history_archive, &document_only_bytes)
        .expect_err("stage verification must reject silently dropped history");
    assert_eq!(error, "generated .ori2 data did not round-trip exactly");
}

#[test]
fn document_snapshot_keeps_identity_name_and_dirty_state() {
    let mut document = ProjectDocument::new("Loaded bird", CreasePattern::empty());
    document.memo = "Check the reverse side.".to_owned();
    document.paper.cutting_allowed = true;
    let project = ProjectState::from_valid_document(document.clone(), PathBuf::from("bird.ori2"));
    let response = snapshot(&project);

    assert_eq!(response.project_id, document.project_id);
    assert_eq!(response.name, "Loaded bird");
    assert_eq!(response.memo, "Check the reverse side.");
    assert_eq!(response.current_path.as_deref(), Some("bird.ori2"));
    assert!(!response.is_dirty);
    assert_eq!(response.paper, document.paper);
    assert!(response.cutting_allowed);
    assert!(!response.can_undo);
    let persisted = project.document();
    assert!(persisted.thumbnail_svg.is_some());
    document.thumbnail_svg = persisted.thumbnail_svg.clone();
    assert_eq!(persisted, document);
}

#[test]
fn project_memo_is_dirty_undoable_and_round_trips_through_history() {
    let mut project = ProjectState::new(CreasePattern::empty());
    project
        .editor
        .execute(
            0,
            Command::UpdateProjectMemo {
                memo: "First draft".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(project.document().memo, "First draft");
    assert!(project.is_dirty());

    let archive = project.project_archive().unwrap();
    let mut reopened =
        ProjectState::from_project_archive(archive, PathBuf::from("memo.ori2")).unwrap();
    assert_eq!(reopened.document().memo, "First draft");
    reopened.editor.undo(reopened.editor.revision()).unwrap();
    assert!(reopened.document().memo.is_empty());
    reopened.editor.redo(reopened.editor.revision()).unwrap();
    assert_eq!(reopened.document().memo, "First draft");
}

#[test]
fn stale_project_identity_cannot_mutate_a_replacement_project() {
    let mut project = ProjectState::new(CreasePattern::empty());
    let stale_project_id = ProjectId::new();

    let error = execute_command(
        &mut project,
        stale_project_id,
        0,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(1.0, 1.0),
        },
    )
    .expect_err("a command for another project must fail");

    assert_eq!(
        error,
        "the active project changed before the command was applied"
    );
    assert!(project.editor.pattern().vertices.is_empty());
}

#[test]
fn undoing_to_saved_content_clears_dirty_state() {
    let vertex_id = VertexId::new();
    let document = ProjectDocument::new(
        "Saved bird",
        CreasePattern {
            vertices: vec![Vertex {
                id: vertex_id,
                position: Point2::new(1.0, 2.0),
            }],
            edges: Vec::new(),
        },
    );
    let mut project = ProjectState::from_valid_document(document, PathBuf::from("bird.ori2"));
    let project_id = project.project_id;

    execute_command(
        &mut project,
        project_id,
        0,
        Command::MoveVertex {
            id: vertex_id,
            position: Point2::new(3.0, 4.0),
        },
    )
    .expect("move vertex");
    assert!(project.is_dirty());

    project.editor.undo(1).expect("undo to save point");
    assert!(!project.is_dirty());
}

#[test]
fn undoing_a_removal_to_saved_order_clears_dirty_state() {
    let vertices = [
        Vertex {
            id: VertexId::new(),
            position: Point2::new(0.0, 0.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(1.0, 0.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(2.0, 0.0),
        },
    ];
    let document = ProjectDocument::new(
        "Saved bird",
        CreasePattern {
            vertices: vertices.to_vec(),
            edges: Vec::new(),
        },
    );
    let mut project = ProjectState::from_valid_document(document, PathBuf::from("bird.ori2"));
    let project_id = project.project_id;

    execute_command(
        &mut project,
        project_id,
        0,
        Command::RemoveVertex { id: vertices[1].id },
    )
    .expect("remove middle vertex");
    assert!(project.is_dirty());

    project.editor.undo(1).expect("undo to saved ordering");
    assert!(!project.is_dirty());
}
