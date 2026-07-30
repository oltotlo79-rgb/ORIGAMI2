#[test]
fn loaded_current_instruction_poses_are_semantically_checked_but_stale_ones_survive() {
    let project = initial_project_state();
    let mut document = project.document();
    let current_fingerprint = project.editor.fold_model_fingerprint_v1();
    let invalid_current_step = InstructionStep {
        id: InstructionStepId::new(),
        title: "不正な現在姿勢".to_owned(),
        description: String::new(),
        caution: String::new(),
        duration_ms: 1_000,
        visual: Default::default(),
        pose: InstructionPose {
            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
            source_model_fingerprint: current_fingerprint,
            fixed_face: Some(FaceId::new()),
            hinge_angles: Vec::new(),
        },
    };
    document
        .instruction_timeline
        .steps
        .push(invalid_current_step.clone());

    assert_eq!(
        validate_document_instruction_poses(&document)
            .expect_err("current malformed pose must fail semantic loading"),
        "instruction step 1 is invalid: a planar instruction pose must not specify a fixed face"
    );

    document.instruction_timeline.steps[0]
        .pose
        .source_model_fingerprint = "f".repeat(64);
    validate_document_instruction_poses(&document)
        .expect("an old-model pose remains loadable as an editable stale step");
}

#[test]
fn delayed_instruction_pose_cannot_land_after_reopening_the_same_document() {
    let project = initial_project_state();
    let project_id = project.project_id;
    let input =
        capture_topology_input(&project, project_id, 0).expect("capture instruction topology");
    let topology = input.analyze();
    let analyzed = AnalyzedInstructionPose {
        project_instance_id: project.instance_id,
        input,
        topology,
        fixed_face: None,
        hinge_angles: Vec::new(),
    };

    let reopened =
        ProjectState::from_valid_document(project.document(), PathBuf::from("same-project.ori2"));
    assert_eq!(reopened.project_id, project_id);
    assert_eq!(reopened.editor.revision(), 0);
    assert_eq!(reopened.editor.pattern(), project.editor.pattern());
    assert_eq!(reopened.editor.paper(), project.editor.paper());
    assert_ne!(reopened.instance_id, project.instance_id);
    let before = project_state_signature(&reopened);

    assert_eq!(
        super::finish_instruction_pose(&reopened, reopened.instance_id, project_id, 0, analyzed,)
            .expect_err("an old open-instance analysis must not mutate the reopened project"),
        "the open project instance changed while the instruction pose was being analyzed"
    );
    assert_eq!(project_state_signature(&reopened), before);
}

#[test]
fn instruction_pose_capture_rejects_same_document_revision_after_reopen_aba() {
    let project = initial_project_state();
    let stale_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let reopened =
        ProjectState::from_valid_document(project.document(), PathBuf::from("same-project.ori2"));
    assert_eq!(reopened.project_id, expected_project_id);
    assert_eq!(reopened.editor.revision(), expected_revision);
    assert_ne!(reopened.instance_id, stale_instance_id);
    let state = AppState::new(reopened);
    let before = {
        let project = lock_project(&state).expect("lock reopened project");
        project_state_signature(&project)
    };

    let result = tauri::async_runtime::block_on(analyze_instruction_pose(
        &state,
        stale_instance_id,
        expected_project_id,
        expected_revision,
        None,
        Vec::new(),
    ));
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("reopened ABA instance must reject delayed instruction analysis"),
    };

    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    let project = lock_project(&state).expect("lock unchanged reopened project");
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn semantic_instruction_failure_cannot_overwrite_an_existing_save() {
    let project = initial_project_state();
    let mut document = project.document();
    document.instruction_timeline.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: "不正な現在姿勢".to_owned(),
        description: String::new(),
        caution: String::new(),
        duration_ms: 1_000,
        visual: Default::default(),
        pose: InstructionPose {
            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
            source_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
            fixed_face: Some(FaceId::new()),
            hinge_angles: Vec::new(),
        },
    });
    let directory = TestDirectory::new();
    let path = directory.join("existing.ori2");
    let original = b"existing project bytes";
    fs::write(&path, original).expect("create existing target");

    let error = persist_document(&path, &document)
        .expect_err("semantic validation must run before staging a save");

    assert_eq!(error, PROJECT_INSTRUCTIONS_SAVE_FAILED_MESSAGE);
    assert!(!error.contains("不正な現在姿勢"));
    assert_eq!(fs::read(&path).expect("read preserved target"), original);
    assert_eq!(
        fs::read_dir(&directory.path)
            .expect("inspect save directory")
            .count(),
        1,
        "semantic rejection must not leave a staged file"
    );
}

#[test]
fn topology_bridge_preserves_three_faces_and_two_hinges_for_multiple_folds() {
    let project = cellular_multi_fold_project_state();
    let before = project_state_signature(&project);
    let input = capture_topology_input(&project, project.project_id, 0)
        .expect("capture cellular fold graph");

    let response = finish_topology_response(&project, &input, input.analyze())
        .expect("finish cellular fold topology");

    assert!(response.simulation_ready);
    assert!(response.issues.is_empty());
    let snapshot = response.snapshot.expect("cellular fold snapshot");
    assert_eq!(snapshot.source_revision, 0);
    assert_eq!(snapshot.faces.len(), 3);
    assert_eq!(snapshot.hinge_adjacency.len(), 2);
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn topology_bridge_preserves_structured_unsupported_diagnostics() {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("cut-enabled sheet");
    let (pattern, paper) = sheet.into_parts();
    let mut project = ProjectState::new_with_paper(pattern, paper);
    let boundary = project.editor.paper().boundary_vertices.clone();
    let cut = EdgeId::new();
    let project_id = project.project_id;
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddEdge {
            id: cut,
            start: boundary[0],
            end: boundary[2],
            kind: EdgeKind::Cut,
        },
    )
    .expect("add supported editor cut");
    let input = capture_topology_input(&project, project_id, 1).expect("capture unsupported graph");

    let response = finish_topology_response(&project, &input, input.analyze())
        .expect("unsupported topology is a diagnostic response");

    assert!(!response.simulation_ready);
    assert!(response.snapshot.is_none());
    assert!(matches!(
        response.issues.as_slice(),
        [TopologyIssue {
            kind: ori_core::TopologyIssueKind::UnsupportedActiveEdge {
                edge,
                edge_kind: EdgeKind::Cut,
            },
            ..
        }] if *edge == cut
    ));
}

#[test]
fn topology_bridge_rejects_stale_capture_and_delayed_aba_result() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    assert_eq!(
        capture_topology_input(&project, project_id, 1).expect_err("stale requested revision"),
        "expected revision 1, but the current revision is 0"
    );
    assert!(capture_topology_input(&project, ProjectId::new(), 0).is_err());

    let input = capture_topology_input(&project, project_id, 0).expect("capture old input");
    let topology = input.analyze();
    let replacement = create_rectangular_sheet(210.0, 297.0, false).expect("replacement rectangle");
    let (pattern, paper) = replacement.into_parts();
    project.editor = EditorState::with_paper(pattern, paper);
    assert_eq!(project.editor.revision(), 0, "ABA revision fixture");

    assert_eq!(
        finish_topology_response(&project, &input, topology)
            .expect_err("same identity/revision with different content is stale"),
        "the project changed while topology was being analyzed"
    );
}

#[test]
fn validation_worker_releases_project_lock_during_exact_analysis() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let worker_state = Arc::clone(&state);
    let (entered_tx, entered_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);

    let validation = thread::spawn(move || {
        tauri::async_runtime::block_on(validate_project_with_worker(&worker_state, move |input| {
            entered_tx.send(()).expect("announce worker entry");
            release_rx.recv().expect("release validation worker");
            Ok(analyze_validation_input(input))
        }))
    });

    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("validation worker must start");
    let lock_was_available = state.0.try_lock().is_ok();
    release_tx.send(()).expect("release validation worker");

    let snapshot = validation
        .join()
        .expect("validation caller thread must not panic")
        .expect("unchanged validation must finish");
    assert!(
        lock_was_available,
        "the project mutex must not be held during exact validation"
    );
    assert_eq!(snapshot.revision, 0);
}

#[test]
fn validation_worker_rejects_same_revision_aba_content() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let worker_state = Arc::clone(&state);
    let (entered_tx, entered_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);

    let validation = thread::spawn(move || {
        tauri::async_runtime::block_on(validate_project_with_worker(&worker_state, move |input| {
            entered_tx.send(()).expect("announce worker entry");
            release_rx.recv().expect("release validation worker");
            Ok(analyze_validation_input(input))
        }))
    });

    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("validation worker must start");
    {
        let Ok(mut project) = state.0.try_lock() else {
            release_tx
                .send(())
                .expect("release blocked validation worker");
            validation
                .join()
                .expect("validation caller thread must not panic")
                .expect("unchanged validation must finish");
            panic!("the project mutex must be available while validation is running");
        };
        let replacement =
            create_rectangular_sheet(210.0, 297.0, false).expect("replacement rectangle");
        let (pattern, paper) = replacement.into_parts();
        project.editor = EditorState::with_paper(pattern, paper);
        assert_eq!(project.editor.revision(), 0, "ABA revision fixture");
    }
    release_tx.send(()).expect("release validation worker");

    let error = validation
        .join()
        .expect("validation caller thread must not panic")
        .expect_err("same-revision replacement must make the result stale");
    assert_eq!(
        error,
        "the project changed while validation was being analyzed"
    );
}

#[test]
fn validation_worker_panic_and_reported_failure_are_redacted_and_fail_closed() {
    let state = AppState::new(initial_project_state());
    let private_panic = r"C:\Users\alice\秘密の作品.ori2 at vertex=(12.3,45.6)";

    let panic_error = tauri::async_runtime::block_on(validate_project_with_worker(
        &state,
        move |_| -> Result<AnalyzedProjectValidation, String> {
            panic!("{private_panic}");
        },
    ))
    .expect_err("a panicking worker must fail the command");
    assert_eq!(panic_error, VALIDATION_ANALYSIS_FAILED_MESSAGE);
    assert!(!panic_error.contains("alice"));
    assert!(!panic_error.contains("秘密の作品"));
    assert!(!panic_error.contains("12.3"));

    let private_failure = r"C:\Users\bob\非公開.ori2; internal_id=validation-7";
    let reported_error =
        tauri::async_runtime::block_on(validate_project_with_worker(&state, move |_| {
            Err(private_failure.to_owned())
        }))
        .expect_err("a reported worker failure must fail the command");
    assert_eq!(reported_error, VALIDATION_ANALYSIS_FAILED_MESSAGE);
    assert!(!reported_error.contains("bob"));
    assert!(!reported_error.contains("非公開"));
    assert!(!reported_error.contains("validation-7"));
    assert!(
        state.0.try_lock().is_ok(),
        "worker failures must not poison or retain the project mutex"
    );
}

#[test]
fn background_task_failures_discard_private_panic_payloads() {
    let private_payload = r"C:\Users\alice\秘密の作品.ori2; face_id=private; point=(12.3,45.6)";
    let errors = [
        topology_analysis_task_error(private_payload),
        instruction_topology_analysis_task_error(private_payload),
        fold_import_task_error(private_payload),
        fold_conversion_task_error(private_payload),
    ];

    assert_eq!(
        errors,
        [
            TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE,
            INSTRUCTION_TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE,
            FOLD_IMPORT_TASK_FAILED_MESSAGE,
            FOLD_CONVERSION_TASK_FAILED_MESSAGE,
        ]
    );
    for error in errors {
        assert!(!error.contains("alice"));
        assert!(!error.contains("秘密の作品"));
        assert!(!error.contains("face_id"));
        assert!(!error.contains("12.3"));
    }
}

fn unsaved_project_with_redo_history(name: &str) -> ProjectState {
    let mut project =
        ProjectState::new_unsaved(name.to_owned(), CreasePattern::empty(), Paper::default());
    let project_id = project.project_id;
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(12.0, 34.0),
        },
    )
    .expect("add history fixture vertex");
    project.editor.undo(1).expect("create redo history");
    assert!(project.editor.can_redo());
    project
}

fn unsaved_project_with_undo_and_redo_history(name: &str) -> (ProjectState, VertexId, VertexId) {
    let mut project =
        ProjectState::new_unsaved(name.to_owned(), CreasePattern::empty(), Paper::default());
    project
        .editor
        .set_history_entry_limit(17)
        .expect("configure persisted history limit");
    let project_id = project.project_id;
    let first = VertexId::new();
    let second = VertexId::new();
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddVertex {
            id: first,
            position: Point2::new(12.0, 34.0),
        },
    )
    .expect("add first history fixture vertex");
    execute_command(
        &mut project,
        project_id,
        1,
        Command::AddVertex {
            id: second,
            position: Point2::new(56.0, 78.0),
        },
    )
    .expect("add second history fixture vertex");
    project
        .editor
        .undo(2)
        .expect("leave both Undo and Redo stacks populated");
    assert!(project.editor.can_undo());
    assert!(project.editor.can_redo());
    (project, first, second)
}

fn project_with_reachable_invalid_instruction_pose(name: &str) -> ProjectState {
    let sheet = create_rectangular_sheet(40.0, 40.0, false).expect("valid history test sheet");
    let (pattern, paper) = sheet.into_parts();
    let mut project = ProjectState::new_unsaved(name.to_owned(), pattern, paper);
    let project_id = project.project_id;
    let old_fingerprint = project.editor.fold_model_fingerprint_v1();
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddInstructionStep {
            step: InstructionStep {
                id: InstructionStepId::new(),
                title: "invalid only after Undo".to_owned(),
                description: String::new(),
                caution: String::new(),
                duration_ms: 1_000,
                visual: Default::default(),
                pose: InstructionPose {
                    model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                    source_model_fingerprint: old_fingerprint.clone(),
                    fixed_face: Some(FaceId::new()),
                    hinge_angles: Vec::new(),
                },
            },
        },
    )
    .expect("the editor accepts structurally valid pose metadata");
    execute_command(
        &mut project,
        project_id,
        1,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(20.0, 20.0),
        },
    )
    .expect("make the invalid instruction pose stale in the current document");
    assert_ne!(project.editor.fold_model_fingerprint_v1(), old_fingerprint);
    assert!(
        validate_document_instruction_poses(&project.document()).is_ok(),
        "the final stale pose is intentionally accepted"
    );
    let mut undo_endpoint = project.editor.clone();
    undo_endpoint.undo(2).expect("reach old model endpoint");
    let mut endpoint_document = project.document();
    endpoint_document.paper = undo_endpoint.paper().clone();
    endpoint_document.crease_pattern = undo_endpoint.pattern().clone();
    endpoint_document.instruction_timeline = undo_endpoint.instruction_timeline().clone();
    endpoint_document.geometric_constraints = undo_endpoint.geometric_constraints().clone();
    endpoint_document.layers = undo_endpoint.project_layers().clone();
    assert!(
        validate_document_instruction_poses(&endpoint_document).is_err(),
        "the same pose becomes current and invalid after Undo"
    );
    project
}

fn project_with_redo_reachable_invalid_instruction_pose(name: &str) -> ProjectState {
    let sheet = create_rectangular_sheet(40.0, 40.0, false).expect("valid history test sheet");
    let (pattern, paper) = sheet.into_parts();
    let mut project = ProjectState::new_unsaved(name.to_owned(), pattern, paper);
    let project_id = project.project_id;
    let fingerprint = project.editor.fold_model_fingerprint_v1();
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddInstructionStep {
            step: InstructionStep {
                id: InstructionStepId::new(),
                title: "invalid only after Redo".to_owned(),
                description: String::new(),
                caution: String::new(),
                duration_ms: 1_000,
                visual: Default::default(),
                pose: InstructionPose {
                    model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                    source_model_fingerprint: fingerprint,
                    fixed_face: Some(FaceId::new()),
                    hinge_angles: Vec::new(),
                },
            },
        },
    )
    .expect("the editor accepts structurally valid pose metadata");
    project
        .editor
        .undo(1)
        .expect("leave the invalid step only on the Redo endpoint");
    assert!(project.editor.instruction_timeline().steps.is_empty());
    assert!(project.editor.can_redo());
    assert!(validate_document_instruction_poses(&project.document()).is_ok());
    project
}

fn corrupt_editor_history_payload(mut bytes: Vec<u8>) -> Vec<u8> {
    const LOCAL_FILE_HEADER_SIZE: usize = 30;
    const HISTORY_PATH: &[u8] = b"editor-history.json";
    let name_start = bytes
        .windows(HISTORY_PATH.len())
        .position(|window| window == HISTORY_PATH)
        .expect("history local-header name");
    let header_start = name_start
        .checked_sub(LOCAL_FILE_HEADER_SIZE)
        .expect("history local-header offset");
    assert_eq!(
        &bytes[header_start..header_start + 4],
        b"PK\x03\x04",
        "the first history path must belong to its local ZIP header"
    );
    let compressed_size = u32::from_le_bytes(
        bytes[header_start + 18..header_start + 22]
            .try_into()
            .expect("compressed-size field"),
    ) as usize;
    let extra_length = u16::from_le_bytes(
        bytes[header_start + 28..header_start + 30]
            .try_into()
            .expect("extra-length field"),
    ) as usize;
    assert!(compressed_size > 0);
    let payload_start = name_start + HISTORY_PATH.len() + extra_length;
    let corrupt_at = payload_start + compressed_size / 2;
    bytes[corrupt_at] ^= 0x01;
    bytes
}

fn tamper_ori2_project_certificate(bytes: &[u8], reauthenticate_manifest: bool) -> Vec<u8> {
    let mut source = ZipArchive::new(Cursor::new(bytes)).expect("open source ORI2");
    let mut entries = Vec::with_capacity(source.len());
    for index in 0..source.len() {
        let mut entry = source.by_index(index).expect("read source ORI2 entry");
        let name = entry.name().to_owned();
        let mut payload = Vec::new();
        entry.read_to_end(&mut payload).expect("read ORI2 payload");
        entries.push((name, entry.compression(), payload));
    }
    let project_payload = &mut entries
        .iter_mut()
        .find(|(name, _, _)| name == ori_formats::PROJECT_FOLDER_PROJECT_PATH)
        .expect("ORI2 project entry")
        .2;
    let mut project: serde_json::Value = serde_json::from_slice(project_payload).unwrap();
    let certificate_byte = project
        .pointer_mut(
            "/beginner_design_profile/generation_provenance/fold_path_certificate_sha256/0",
        )
        .expect("generic tree certificate byte");
    *certificate_byte = serde_json::json!(certificate_byte.as_u64().unwrap_or_default() ^ 1);
    *project_payload = serde_json::to_vec(&project).unwrap();
    if reauthenticate_manifest {
        let project_size = project_payload.len() as u64;
        let project_sha256 = sha2::Sha256::digest(project_payload.as_slice())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let manifest_payload = &mut entries
            .iter_mut()
            .find(|(name, _, _)| name == ori_formats::PROJECT_FOLDER_MANIFEST_PATH)
            .expect("ORI2 manifest entry")
            .2;
        let mut manifest: serde_json::Value = serde_json::from_slice(manifest_payload).unwrap();
        let descriptor = manifest
            .get_mut("project")
            .expect("ORI2 project descriptor");
        descriptor["uncompressed_size"] = serde_json::json!(project_size);
        descriptor["sha256"] = serde_json::json!(project_sha256);
        *manifest_payload = serde_json::to_vec(&manifest).unwrap();
    }
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, compression, payload) in entries {
        writer
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(compression),
            )
            .expect("start tampered ORI2 entry");
        writer
            .write_all(&payload)
            .expect("write tampered ORI2 entry");
    }
    writer.finish().expect("finish tampered ORI2").into_inner()
}
