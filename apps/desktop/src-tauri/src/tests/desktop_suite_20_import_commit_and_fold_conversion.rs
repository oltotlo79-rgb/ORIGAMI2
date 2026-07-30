#[test]
fn fold_import_commit_is_an_atomic_new_unsaved_project_replacement() {
    let mut project = unsaved_project_with_redo_history("Existing project");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let import_id = ProjectId::new();
    let mut pending = Some(PendingFoldImport {
        import_id,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        bytes: Arc::from(br#"{"file_spec":1.2}"#.as_slice()),
    });
    let replacement = create_new_project_state(new_project_parameters())
        .expect("create import replacement fixture");
    let replacement_project_id = replacement.project_id;
    let replacement_instance_id = replacement.instance_id;

    let response = commit_fold_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        replacement,
    )
    .expect("commit current import");

    assert_eq!(response.project_id, replacement_project_id);
    assert_eq!(project.instance_id, replacement_instance_id);
    assert_ne!(project.project_id, expected_project_id);
    assert_eq!(project.editor.revision(), 0);
    assert!(!project.editor.can_undo());
    assert!(!project.editor.can_redo());
    assert!(project.current_path.is_none());
    assert!(project.saved_revision.is_none());
    assert!(project.saved_document.is_none());
    assert!(project.is_dirty());
    assert!(pending.is_none());
}

#[test]
fn svg_import_commit_is_an_atomic_new_unsaved_project_replacement() {
    let mut project = unsaved_project_with_redo_history("Existing project");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let import_id = ProjectId::new();
    let mut pending = Some(PendingSvgImport {
        import_id,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        bytes: Arc::from(br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.as_slice()),
    });
    let replacement = create_new_project_state(new_project_parameters())
        .expect("create SVG import replacement fixture");

    let before = project_state_signature(&project);
    let error = commit_svg_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        false,
        replacement,
    )
    .expect_err("dirty SVG replacement must require confirmation");
    assert!(error.contains("explicit confirmation"));
    assert_eq!(project_state_signature(&project), before);
    assert!(pending.is_some());

    let replacement = create_new_project_state(new_project_parameters())
        .expect("create confirmed SVG import replacement fixture");
    let replacement_project_id = replacement.project_id;
    let replacement_instance_id = replacement.instance_id;
    let response = commit_svg_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        true,
        replacement,
    )
    .expect("commit current SVG import");

    assert_eq!(response.project_id, replacement_project_id);
    assert_eq!(project.instance_id, replacement_instance_id);
    assert_ne!(project.project_id, expected_project_id);
    assert_eq!(project.editor.revision(), 0);
    assert!(!project.editor.can_undo());
    assert!(!project.editor.can_redo());
    assert!(project.current_path.is_none());
    assert!(project.saved_revision.is_none());
    assert!(project.saved_document.is_none());
    assert!(project.is_dirty());
    assert!(pending.is_none());
}

#[test]
fn svg_import_commit_rejects_revision_and_instance_aba_without_mutation() {
    let mut project = unsaved_project_with_redo_history("Existing project");
    let stale_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let import_id = ProjectId::new();
    let pending_template = PendingSvgImport {
        import_id,
        expected_instance_id: stale_instance_id,
        expected_project_id,
        expected_revision,
        bytes: Arc::from(br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.as_slice()),
    };

    project
        .editor
        .execute(
            expected_revision,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(12.0, 13.0),
            },
        )
        .expect("edit after SVG preview");
    let revision_before = project_state_signature(&project);
    let mut pending = Some(pending_template.clone());
    let error = commit_svg_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        true,
        create_new_project_state(new_project_parameters()).unwrap(),
    )
    .expect_err("stale SVG revision must fail");
    assert_eq!(error, "the project changed while the file dialog was open");
    assert_eq!(project_state_signature(&project), revision_before);
    assert!(pending.is_some());

    let document = project.document();
    project = ProjectState::from_valid_document(document, PathBuf::from("same.ori2"));
    project.project_id = expected_project_id;
    assert_ne!(project.instance_id, stale_instance_id);
    let instance_before = project_state_signature(&project);
    let mut pending = Some(pending_template);
    let error = commit_svg_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        true,
        create_new_project_state(new_project_parameters()).unwrap(),
    )
    .expect_err("reopened project instance must fail");
    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&project), instance_before);
    assert!(pending.is_some());
}

#[test]
fn fold_import_commit_rejects_revision_and_instance_aba_without_mutation() {
    let mut project = unsaved_project_with_redo_history("Existing project");
    let stale_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let import_id = ProjectId::new();
    let pending_template = PendingFoldImport {
        import_id,
        expected_instance_id: stale_instance_id,
        expected_project_id,
        expected_revision,
        bytes: Arc::from(br#"{"file_spec":1.2}"#.as_slice()),
    };

    project
        .editor
        .execute(
            expected_revision,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(12.0, 13.0),
            },
        )
        .expect("edit after preview");
    let revision_before = project_state_signature(&project);
    let mut pending = Some(pending_template.clone());
    let error = commit_fold_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        create_new_project_state(new_project_parameters()).unwrap(),
    )
    .expect_err("stale revision must fail");
    assert_eq!(error, "the project changed while the file dialog was open");
    assert_eq!(project_state_signature(&project), revision_before);
    assert!(pending.is_some());

    let document = project.document();
    project = ProjectState::from_valid_document(document, PathBuf::from("same.ori2"));
    project.project_id = expected_project_id;
    assert_ne!(project.instance_id, stale_instance_id);
    let instance_before = project_state_signature(&project);
    let mut pending = Some(pending_template);
    let error = commit_fold_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        create_new_project_state(new_project_parameters()).unwrap(),
    )
    .expect_err("reopened project instance must fail");
    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&project), instance_before);
    assert!(pending.is_some());
}

#[test]
fn fold_import_mapping_and_scale_validation_reject_ambiguous_requests() {
    assert!(validate_import_scale(1.0).is_ok());
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY, 1_000_000_000.000_001] {
        assert!(validate_import_scale(invalid).is_err());
    }

    let valid = validate_fold_import_mapping_requests(vec![
        FoldImportAssignmentMappingRequest {
            source: "M".to_owned(),
            target: FoldImportTargetRequest::Mountain,
        },
        FoldImportAssignmentMappingRequest {
            source: "U".to_owned(),
            target: FoldImportTargetRequest::Valley,
        },
        FoldImportAssignmentMappingRequest {
            source: "J".to_owned(),
            target: FoldImportTargetRequest::Ignore,
        },
    ])
    .expect("validate supported mappings");
    assert_eq!(valid.len(), 3);

    assert!(
        validate_fold_import_mapping_requests(vec![FoldImportAssignmentMappingRequest {
            source: "M".to_owned(),
            target: FoldImportTargetRequest::Valley,
        }])
        .is_err()
    );
    assert!(
        validate_fold_import_mapping_requests(vec![FoldImportAssignmentMappingRequest {
            source: "X".to_owned(),
            target: FoldImportTargetRequest::Ignore,
        }])
        .is_err()
    );
    assert!(
        validate_fold_import_mapping_requests(vec![
            FoldImportAssignmentMappingRequest {
                source: "F".to_owned(),
                target: FoldImportTargetRequest::Auxiliary,
            },
            FoldImportAssignmentMappingRequest {
                source: "F".to_owned(),
                target: FoldImportTargetRequest::Ignore,
            },
        ])
        .is_err()
    );
}

#[test]
fn fold_import_mapping_or_geometry_failure_preserves_project_and_pending_preview() {
    let project = unsaved_project_with_redo_history("Keep this project");
    let before = project_state_signature(&project);
    let valid_bytes = serde_json::to_vec(&serde_json::json!({
        "file_spec": 1.2,
        "frame_unit": "mm",
        "vertices_coords": [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
        "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0], [0, 2]],
        "edges_assignment": ["B", "B", "B", "B", "M"]
    }))
    .expect("serialize mapping fixture");
    let import_id = ProjectId::new();
    let mut pending = Some(PendingFoldImport {
        import_id,
        expected_instance_id: project.instance_id,
        expected_project_id: project.project_id,
        expected_revision: project.editor.revision(),
        bytes: Arc::from(valid_bytes.clone()),
    });

    let mapping_error = build_fold_import_replacement(
        &valid_bytes,
        "Missing mapping".to_owned(),
        1.0,
        FoldBoundaryCandidateId(0),
        HashMap::new(),
    )
    .err()
    .expect("missing M mapping must fail");
    assert!(mapping_error.contains("no mapping was selected"));
    assert_eq!(project_state_signature(&project), before);
    assert_eq!(
        pending.as_ref().map(|value| value.import_id),
        Some(import_id)
    );

    let crossing_bytes = serde_json::to_vec(&serde_json::json!({
        "file_spec": 1.2,
        "frame_unit": "mm",
        "vertices_coords": [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
        "edges_vertices": [
            [0, 1], [1, 2], [2, 3], [3, 0],
            [0, 2], [1, 3]
        ],
        "edges_assignment": ["B", "B", "B", "B", "M", "V"]
    }))
    .expect("serialize crossing fixture");
    let geometry_error = build_fold_import_replacement(
        &crossing_bytes,
        "Crossing".to_owned(),
        1.0,
        FoldBoundaryCandidateId(0),
        HashMap::from([
            ("M".to_owned(), FoldImportTargetRequest::Mountain),
            ("V".to_owned(), FoldImportTargetRequest::Valley),
        ]),
    )
    .err()
    .expect("unsplit crossing must fail final validation");
    assert!(geometry_error.contains("validation issue(s)"));
    assert_eq!(project_state_signature(&project), before);
    assert_eq!(
        pending.as_ref().map(|value| value.import_id),
        Some(import_id)
    );

    let replacement =
        create_new_project_state(new_project_parameters()).expect("create unused replacement");
    // The failed conversion path never reaches the only replacement
    // boundary; retaining this assertion guards accidental future calls.
    assert_ne!(replacement.project_id, project.project_id);
    assert!(pending.take().is_some());
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn fold_import_file_errors_do_not_expose_the_selected_path() {
    let directory = TestDirectory::new();
    let secret_name = "private-client-design.fold";
    let path = directory.join(secret_name);

    let missing_error = read_fold_import_bytes(&path).expect_err("missing import must be rejected");
    assert_eq!(missing_error, FOLD_FILE_OPEN_FAILED_MESSAGE);
    assert!(!missing_error.contains(secret_name));
    assert!(!missing_error.contains(&directory.path.to_string_lossy().into_owned()));
    assert!(!missing_error.to_ascii_lowercase().contains("os error"));

    let private_file_spec = 987_654_321.125_f64;
    let private_value = private_file_spec.to_string();
    let malformed = serde_json::to_vec(&serde_json::json!({
        "file_spec": private_file_spec,
        "vertices_coords": [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0]],
        "edges_assignment": ["B", "B", "B", "B"]
    }))
    .expect("serialize private malformed FOLD fixture");
    fs::write(&path, &malformed).expect("write malformed FOLD fixture");
    let malformed_error =
        load_fold_import_preview(&path).expect_err("unsupported FOLD version must be rejected");
    assert_eq!(malformed_error, FOLD_FILE_INVALID_MESSAGE);
    assert!(!malformed_error.contains(&private_value));
    assert!(!malformed_error.contains(secret_name));

    let staged_error = build_fold_import_replacement(
        &malformed,
        "Private staged input".to_owned(),
        1.0,
        FoldBoundaryCandidateId(0),
        HashMap::new(),
    )
    .err()
    .expect("staged unsupported FOLD version must be rejected");
    assert_eq!(staged_error, FOLD_FILE_INVALID_MESSAGE);
    assert!(!staged_error.contains(&private_value));

    File::create(&path)
        .expect("create oversized fixture")
        .set_len(MAX_FOLD_IMPORT_FILE_SIZE + 1)
        .expect("make sparse oversized fixture");
    let oversized_error =
        read_fold_import_bytes(&path).expect_err("oversized import must be rejected");
    assert_eq!(oversized_error, FOLD_FILE_TOO_LARGE_MESSAGE);
    assert!(!oversized_error.contains(secret_name));
    assert!(!oversized_error.contains(&directory.path.to_string_lossy().into_owned()));
    assert!(!oversized_error.contains(&(MAX_FOLD_IMPORT_FILE_SIZE + 1).to_string()));
}

#[test]
fn fold_import_preview_contract_and_conversion_create_a_valid_editable_project() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "file_spec": 1.2,
        "file_title": "  取込テスト  ",
        "frame_unit": "cm",
        "vertices_coords": [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
        "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0], [0, 2]],
        "edges_assignment": ["B", "B", "B", "B", "M"]
    }))
    .expect("serialize FOLD fixture");
    let preview = read_fold_preview(&bytes).expect("read FOLD preview");
    let import_id = ProjectId::new();
    let response = fold_import_preview_snapshot(import_id, &preview);

    assert_eq!(response.import_id, import_id);
    assert_eq!(response.file_name, FOLD_IMPORT_FILE_LABEL);
    assert_eq!(response.suggested_name, "取込テスト");
    assert_eq!(response.file_spec.as_deref(), Some("1.2"));
    assert_eq!(response.frame_unit.as_deref(), Some("cm"));
    assert_eq!(response.default_mm_per_unit, Some(10.0));
    assert_eq!(response.vertex_count, 4);
    assert_eq!(response.edge_count, 5);
    assert_eq!(response.boundary_edge_count, 4);
    assert_eq!(response.fixed_boundary_candidate_id, Some(0));
    assert_eq!(
        response.boundary_candidates,
        vec![FoldImportBoundaryCandidateSnapshot {
            id: 0,
            source: "assigned_boundary",
            edge_indices: vec![0, 1, 2, 3],
        }]
    );
    assert_eq!(
        response.assignments,
        vec![
            FoldImportAssignmentSummary {
                assignment: "B".to_owned(),
                count: 4,
            },
            FoldImportAssignmentSummary {
                assignment: "M".to_owned(),
                count: 1,
            },
        ]
    );
    assert_eq!(response.preview_vertices.len(), 4);
    assert_eq!(response.preview_edges.len(), 5);
    assert_eq!(
        response
            .preview_edges
            .iter()
            .map(|edge| edge.source_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
    assert!(!response.preview_truncated);
    assert!(response.warnings.is_empty());

    let replacement = build_fold_import_replacement(
        &bytes,
        "取込テスト".to_owned(),
        10.0,
        FoldBoundaryCandidateId(0),
        HashMap::from([("M".to_owned(), FoldImportTargetRequest::Mountain)]),
    )
    .expect("convert FOLD into a project");
    assert_eq!(replacement.name, "取込テスト");
    assert_eq!(replacement.editor.pattern().vertices.len(), 4);
    assert_eq!(replacement.editor.pattern().edges.len(), 5);
    assert_eq!(replacement.editor.paper().boundary_vertices.len(), 4);
    assert!(
        replacement
            .editor
            .pattern()
            .vertices
            .iter()
            .any(|vertex| vertex.position == Point2::new(20.0, 20.0))
    );
    assert_eq!(
        replacement
            .editor
            .pattern()
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Mountain)
            .count(),
        1
    );
    assert!(!replacement.editor.paper().cutting_allowed);
    assert!(replacement.editor.instruction_timeline().steps.is_empty());
    assert_eq!(replacement.editor.revision(), 0);
    assert!(!replacement.editor.can_undo());
    assert!(!replacement.editor.can_redo());
    assert!(replacement.current_path.is_none());
    assert!(replacement.saved_document.is_none());
    assert!(replacement.is_dirty());
}

#[test]
fn fold_import_requires_and_revalidates_an_inferred_boundary_choice() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "file_spec": 1.2,
        "frame_unit": "mm",
        "vertices_coords": [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
        "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0], [0, 2]]
    }))
    .expect("serialize assignment-free FOLD fixture");
    let preview = read_fold_preview(&bytes).expect("read assignment-free FOLD preview");
    let response = fold_import_preview_snapshot(ProjectId::new(), &preview);

    assert_eq!(response.fixed_boundary_candidate_id, None);
    assert_eq!(response.boundary_candidates.len(), 1);
    let candidate = &response.boundary_candidates[0];
    assert_eq!(candidate.source, "inferred_outer_face");
    assert_eq!(candidate.edge_indices, vec![0, 1, 2, 3]);
    assert!(
        response
            .preview_edges
            .iter()
            .filter(|edge| candidate.edge_indices.contains(&edge.source_index))
            .all(|edge| edge.assignment == "U")
    );

    let replacement = build_fold_import_replacement(
        &bytes,
        "外周候補を選択".to_owned(),
        1.0,
        FoldBoundaryCandidateId(candidate.id),
        HashMap::from([("U".to_owned(), FoldImportTargetRequest::Auxiliary)]),
    )
    .expect("convert with the explicitly selected candidate");
    assert_eq!(replacement.editor.paper().boundary_vertices.len(), 4);
    assert_eq!(
        replacement
            .editor
            .pattern()
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Boundary)
            .count(),
        4
    );
    assert_eq!(
        replacement
            .editor
            .pattern()
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Auxiliary)
            .count(),
        1
    );

    let stale_error = match build_fold_import_replacement(
        &bytes,
        "存在しない候補".to_owned(),
        1.0,
        FoldBoundaryCandidateId(candidate.id.saturating_add(1)),
        HashMap::from([("U".to_owned(), FoldImportTargetRequest::Auxiliary)]),
    ) {
        Ok(_) => panic!("an absent candidate ID must be rejected after reparsing"),
        Err(error) => error,
    };
    assert!(stale_error.contains("is not present in this preview"));
}

#[test]
fn fold_import_rejects_an_active_edge_outside_the_paper() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "file_spec": 1.2,
        "frame_unit": "mm",
        "vertices_coords": [
            [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0],
            [2.0, 0.0], [2.0, 1.0]
        ],
        "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0], [4, 5]],
        "edges_assignment": ["B", "B", "B", "B", "M"]
    }))
    .expect("serialize outside-edge fixture");

    let error = build_fold_import_replacement(
        &bytes,
        "紙外の折り線".to_owned(),
        1.0,
        FoldBoundaryCandidateId(0),
        HashMap::from([("M".to_owned(), FoldImportTargetRequest::Mountain)]),
    )
    .err()
    .expect("an active edge outside the paper must be rejected");

    assert!(error.contains("active edge(s) outside the paper boundary"));
}
