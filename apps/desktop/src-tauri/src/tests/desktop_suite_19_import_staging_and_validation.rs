#[test]
fn fold_import_staging_keeps_only_the_latest_preview_and_cancel_is_scoped() {
    let state = FoldImportState::default();
    let project = initial_project_state();
    let first = stage_pending_fold_import(
        &state,
        project.instance_id,
        project.project_id,
        project.editor.revision(),
        br#"{"file_spec":1.2}"#.to_vec(),
    )
    .expect("stage first import");
    let second = stage_pending_fold_import(
        &state,
        project.instance_id,
        project.project_id,
        project.editor.revision(),
        br#"{"file_spec":1.2,"file_title":"newer"}"#.to_vec(),
    )
    .expect("stage replacement import");

    assert_ne!(first, second);
    assert!(pending_fold_import(&state, first, project.project_id, 0).is_err());
    assert_eq!(
        cancel_pending_fold_import(&state, first).unwrap_err(),
        "the FOLD import preview was replaced by a newer preview"
    );
    assert!(pending_fold_import(&state, second, project.project_id, 0).is_ok());
    cancel_pending_fold_import(&state, second).expect("cancel current import");
    cancel_pending_fold_import(&state, second).expect("cancel remains idempotent");
    assert!(lock_fold_import(&state).unwrap().is_none());
}

#[test]
fn svg_import_staging_keeps_only_the_latest_preview_and_cancel_is_scoped() {
    let state = SvgImportState::default();
    let project = initial_project_state();
    let first = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        project.editor.revision(),
        br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.to_vec(),
    )
    .expect("stage first SVG import");
    let second = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        project.editor.revision(),
        br#"<svg xmlns="http://www.w3.org/2000/svg"><title>newer</title></svg>"#.to_vec(),
    )
    .expect("stage replacement SVG import");

    assert_ne!(first, second);
    assert!(pending_svg_import(&state, first, project.project_id, 0).is_err());
    assert_eq!(
        cancel_pending_svg_import(&state, first).unwrap_err(),
        "the SVG import preview was replaced by a newer preview"
    );
    assert!(pending_svg_import(&state, second, project.project_id, 0).is_ok());
    cancel_pending_svg_import(&state, second).expect("cancel current import");
    cancel_pending_svg_import(&state, second).expect("cancel remains idempotent");
    assert!(lock_svg_import(&state).unwrap().pending.is_none());
    assert!(cancel_pending_svg_import(&state, ProjectId::new()).is_err());
}

#[test]
fn svg_import_settings_validation_returns_exact_dimensions_without_replacing_project() {
    let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">
              <rect x="0" y="0" width="100" height="50"
                    fill="none" stroke="#222" data-origami-kind="boundary"/>
              <line x1="0" y1="25" x2="100" y2="25"
                    stroke="#111" data-origami-kind="cut"/>
            </svg>"##;
    let preview = read_svg_preview(bytes).expect("read validation fixture");
    let mut mappings = preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: match group.semantic.as_deref() {
                Some("boundary") => SvgGroupTarget::Boundary,
                Some("cut") => SvgGroupTarget::Cut,
                _ => SvgGroupTarget::Ignore,
            },
        })
        .collect::<Vec<_>>();
    mappings.sort_by_key(|mapping| mapping.group);

    let state = SvgImportState::default();
    let project = initial_project_state();
    let project_before = project_state_signature(&project);
    let import_id = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        project.editor.revision(),
        bytes.to_vec(),
    )
    .expect("stage validation fixture");
    let validation_id = ProjectId::new();
    let pending = begin_svg_import_settings_validation(
        &state,
        validation_id,
        import_id,
        project.project_id,
        project.editor.revision(),
    )
    .expect("begin validation");
    let geometry = validate_svg_import_geometry(&pending.bytes, 2.0, mappings.clone(), None)
        .expect("validate boundary-group geometry");

    let response = {
        let mut slot = lock_svg_import(&state).expect("lock validation state");
        let response = complete_svg_import_settings_validation(
            &mut slot,
            &project,
            SvgImportSettingsValidationCompletion {
                validation: SvgImportSettingsValidation {
                    validation_id,
                    import_id: pending.import_id,
                    expected_instance_id: pending.expected_instance_id,
                    expected_project_id: pending.expected_project_id,
                    expected_revision: pending.expected_revision,
                    millimeters_per_unit_bits: 2.0_f64.to_bits(),
                    boundary_candidate: None,
                    group_mappings: mappings.clone(),
                },
                geometry,
            },
        )
        .expect("complete validation");
        let current = pending_svg_import_in_slot(&slot, import_id, project.project_id, 0).unwrap();
        ensure_svg_import_settings_validation(&slot, current, validation_id, None, 2.0, &mappings)
            .expect("bind validation to exact settings");
        assert!(
            slot.pending.is_some(),
            "validation must retain staged bytes"
        );
        response
    };

    assert_eq!(response.validation_id, validation_id);
    assert_eq!(response.preview_id, import_id);
    assert_eq!(response.expected_project_id, project.project_id);
    assert_eq!(response.expected_revision, 0);
    assert_eq!(response.millimeters_per_unit, 2.0);
    assert_eq!(response.boundary_candidate_id, None);
    assert_eq!(response.width_mm, 200.0);
    assert_eq!(response.height_mm, 100.0);
    assert!(response.has_cuts);
    assert_eq!(project_state_signature(&project), project_before);
}

#[test]
fn svg_import_settings_validation_binds_candidate_and_effective_cut_result() {
    let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">
              <polygon points="0,0 100,0 100,50 0,50"
                       fill="none" stroke="#111" data-origami-kind="cut"/>
            </svg>"##;
    let preview = read_svg_preview(bytes).expect("read candidate fixture");
    let candidate = preview
        .boundary_candidates()
        .iter()
        .find(|candidate| candidate.kind == SvgBoundaryCandidateKind::Polygon)
        .expect("polygon candidate");
    let mappings = preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: SvgGroupTarget::Cut,
        })
        .collect::<Vec<_>>();
    let snapshot =
        svg_import_preview_snapshot(ProjectId::new(), &preview).expect("build candidate snapshot");
    assert!(
        snapshot
            .boundary_candidates
            .iter()
            .any(|candidate| candidate.kind == "polygon")
    );

    let state = SvgImportState::default();
    let project = initial_project_state();
    let import_id = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        0,
        bytes.to_vec(),
    )
    .expect("stage candidate fixture");
    let validation_id = ProjectId::new();
    let pending = begin_svg_import_settings_validation(
        &state,
        validation_id,
        import_id,
        project.project_id,
        0,
    )
    .expect("begin candidate validation");
    let geometry =
        validate_svg_import_geometry(&pending.bytes, 1.0, mappings.clone(), Some(candidate.id))
            .expect("validate selected polygon");
    let response = {
        let mut slot = lock_svg_import(&state).unwrap();
        complete_svg_import_settings_validation(
            &mut slot,
            &project,
            SvgImportSettingsValidationCompletion {
                validation: SvgImportSettingsValidation {
                    validation_id,
                    import_id: pending.import_id,
                    expected_instance_id: pending.expected_instance_id,
                    expected_project_id: pending.expected_project_id,
                    expected_revision: pending.expected_revision,
                    millimeters_per_unit_bits: 1.0_f64.to_bits(),
                    boundary_candidate: Some(candidate.id),
                    group_mappings: mappings,
                },
                geometry,
            },
        )
        .expect("complete candidate validation")
    };

    assert_eq!(response.boundary_candidate_id, Some(candidate.id.0));
    assert_eq!((response.width_mm, response.height_mm), (100.0, 50.0));
    assert!(
        !response.has_cuts,
        "selected source edges become Boundary before effective Cut detection"
    );
}

#[test]
fn svg_import_preview_preserves_every_boundary_candidate_origin() {
    let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"
                              fill="none" stroke="#111">
              <polygon points="0,0 10,0 10,10 0,10"/>
              <polyline points="20,0 30,0 30,10 20,10 20,0"/>
              <rect x="40" y="0" width="10" height="10"/>
              <path d="M 60 0 L 70 0 L 70 10 L 60 10 Z"/>
            </svg>"##;
    let preview = read_svg_preview(bytes).expect("read every candidate origin");
    let snapshot = svg_import_preview_snapshot(ProjectId::new(), &preview)
        .expect("build every candidate origin");
    let kinds = snapshot
        .boundary_candidates
        .iter()
        .map(|candidate| candidate.kind)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        kinds,
        BTreeSet::from([
            "closed_path",
            "polygon",
            "polyline",
            "rectangle",
            "view_box"
        ])
    );
}

#[test]
fn svg_import_settings_validation_rejects_stale_and_superseded_requests() {
    let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg">
              <rect x="0" y="0" width="10" height="20"
                    fill="none" stroke="#222" data-origami-kind="boundary"/>
            </svg>"##;
    let preview = read_svg_preview(bytes).expect("read validation fixture");
    let mappings = preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: SvgGroupTarget::Boundary,
        })
        .collect::<Vec<_>>();
    let state = SvgImportState::default();
    let project = initial_project_state();
    let import_id = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        0,
        bytes.to_vec(),
    )
    .expect("stage validation fixture");

    assert!(
        begin_svg_import_settings_validation(
            &state,
            ProjectId::new(),
            ProjectId::new(),
            project.project_id,
            0,
        )
        .is_err()
    );
    assert!(
        begin_svg_import_settings_validation(
            &state,
            ProjectId::new(),
            import_id,
            project.project_id,
            1,
        )
        .is_err()
    );

    let first_validation_id = ProjectId::new();
    let first = begin_svg_import_settings_validation(
        &state,
        first_validation_id,
        import_id,
        project.project_id,
        0,
    )
    .expect("begin first generation");
    let first_geometry =
        validate_svg_import_geometry(&first.bytes, 1.0, mappings.clone(), None).unwrap();
    let second_validation_id = ProjectId::new();
    let second = begin_svg_import_settings_validation(
        &state,
        second_validation_id,
        import_id,
        project.project_id,
        0,
    )
    .expect("begin second generation");
    {
        let mut slot = lock_svg_import(&state).unwrap();
        assert!(
            complete_svg_import_settings_validation(
                &mut slot,
                &project,
                SvgImportSettingsValidationCompletion {
                    validation: SvgImportSettingsValidation {
                        validation_id: first_validation_id,
                        import_id: first.import_id,
                        expected_instance_id: first.expected_instance_id,
                        expected_project_id: first.expected_project_id,
                        expected_revision: first.expected_revision,
                        millimeters_per_unit_bits: 1.0_f64.to_bits(),
                        boundary_candidate: None,
                        group_mappings: mappings.clone(),
                    },
                    geometry: first_geometry,
                },
            )
            .is_err(),
            "late completion from the old generation must be rejected"
        );
    }
    let second_geometry =
        validate_svg_import_geometry(&second.bytes, 2.0, mappings.clone(), None).unwrap();
    {
        let mut slot = lock_svg_import(&state).unwrap();
        complete_svg_import_settings_validation(
            &mut slot,
            &project,
            SvgImportSettingsValidationCompletion {
                validation: SvgImportSettingsValidation {
                    validation_id: second_validation_id,
                    import_id: second.import_id,
                    expected_instance_id: second.expected_instance_id,
                    expected_project_id: second.expected_project_id,
                    expected_revision: second.expected_revision,
                    millimeters_per_unit_bits: 2.0_f64.to_bits(),
                    boundary_candidate: None,
                    group_mappings: mappings.clone(),
                },
                geometry: second_geometry,
            },
        )
        .expect("complete current generation");
        let pending = pending_svg_import_in_slot(&slot, import_id, project.project_id, 0).unwrap();
        assert!(
            ensure_svg_import_settings_validation(
                &slot,
                pending,
                first_validation_id,
                None,
                2.0,
                &mappings,
            )
            .is_err()
        );
        assert!(
            ensure_svg_import_settings_validation(
                &slot,
                pending,
                second_validation_id,
                None,
                1.0,
                &mappings,
            )
            .is_err(),
            "a changed scale must not reuse old dimensions"
        );
        let mut changed_mappings = mappings.clone();
        changed_mappings[0].target = SvgGroupTarget::Ignore;
        assert!(
            ensure_svg_import_settings_validation(
                &slot,
                pending,
                second_validation_id,
                None,
                2.0,
                &changed_mappings,
            )
            .is_err(),
            "changed mappings must not reuse old dimensions"
        );
    }

    let replacement_id = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        0,
        bytes.to_vec(),
    )
    .expect("stage a newer preview");
    let slot = lock_svg_import(&state).unwrap();
    assert_ne!(replacement_id, import_id);
    assert!(slot.validation.is_none());
    assert!(slot.validation_generation_id.is_none());
}

#[test]
fn svg_import_settings_validation_rejects_a_project_revision_change_without_mutation() {
    let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg">
              <rect x="0" y="0" width="10" height="20"
                    fill="none" stroke="#222" data-origami-kind="boundary"/>
            </svg>"##;
    let preview = read_svg_preview(bytes).expect("read stale revision fixture");
    let mappings = preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: SvgGroupTarget::Boundary,
        })
        .collect::<Vec<_>>();
    let state = SvgImportState::default();
    let mut project = initial_project_state();
    let import_id = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        0,
        bytes.to_vec(),
    )
    .expect("stage stale revision fixture");
    let validation_id = ProjectId::new();
    let pending = begin_svg_import_settings_validation(
        &state,
        validation_id,
        import_id,
        project.project_id,
        0,
    )
    .expect("begin stale revision validation");
    let geometry =
        validate_svg_import_geometry(&pending.bytes, 1.0, mappings.clone(), None).unwrap();
    execute_command(
        &mut project,
        pending.expected_project_id,
        0,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(12.0, 34.0),
        },
    )
    .expect("change project after validation starts");
    let changed_project = project_state_signature(&project);

    {
        let mut slot = lock_svg_import(&state).unwrap();
        assert!(
            complete_svg_import_settings_validation(
                &mut slot,
                &project,
                SvgImportSettingsValidationCompletion {
                    validation: SvgImportSettingsValidation {
                        validation_id,
                        import_id: pending.import_id,
                        expected_instance_id: pending.expected_instance_id,
                        expected_project_id: pending.expected_project_id,
                        expected_revision: pending.expected_revision,
                        millimeters_per_unit_bits: 1.0_f64.to_bits(),
                        boundary_candidate: None,
                        group_mappings: mappings,
                    },
                    geometry,
                },
            )
            .is_err()
        );
        assert!(slot.validation.is_none());
        assert!(slot.pending.is_some());
    }
    abandon_svg_import_settings_validation(&state, validation_id)
        .expect("clear failed validation generation");
    assert_eq!(project_state_signature(&project), changed_project);
}

#[test]
fn svg_import_settings_validation_rejects_invalid_boundaries_and_mappings() {
    let open = br##"<svg xmlns="http://www.w3.org/2000/svg" stroke="#111">
              <line x1="0" y1="0" x2="10" y2="0" data-origami-kind="boundary"/>
              <line x1="10" y1="0" x2="10" y2="10" data-origami-kind="boundary"/>
              <line x1="10" y1="10" x2="0" y2="10" data-origami-kind="boundary"/>
            </svg>"##;
    let open_preview = read_svg_preview(open).expect("read open boundary");
    let open_mappings = open_preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: SvgGroupTarget::Boundary,
        })
        .collect();
    assert!(validate_svg_import_geometry(open, 1.0, open_mappings, None).is_err());

    let multiple = br##"<svg xmlns="http://www.w3.org/2000/svg" stroke="#111">
              <rect x="0" y="0" width="10" height="10"
                    fill="none" data-origami-kind="boundary"/>
              <rect x="20" y="0" width="10" height="10"
                    fill="none" data-origami-kind="boundary"/>
            </svg>"##;
    let multiple_preview = read_svg_preview(multiple).expect("read multiple boundaries");
    let multiple_mappings = multiple_preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: SvgGroupTarget::Boundary,
        })
        .collect();
    assert!(validate_svg_import_geometry(multiple, 1.0, multiple_mappings, None).is_err());

    let valid = br##"<svg xmlns="http://www.w3.org/2000/svg" stroke="#111">
              <rect x="0" y="0" width="10" height="10"
                    fill="none" data-origami-kind="boundary"/>
              <line x1="0" y1="5" x2="10" y2="5" data-origami-kind="mountain"/>
            </svg>"##;
    let valid_preview = read_svg_preview(valid).expect("read complete mapping fixture");
    let boundary_only = valid_preview
        .style_groups()
        .iter()
        .filter(|group| group.semantic.as_deref() == Some("boundary"))
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: SvgGroupTarget::Boundary,
        })
        .collect();
    assert!(
        validate_svg_import_geometry(valid, 1.0, boundary_only, None).is_err(),
        "every retained style group must be mapped"
    );
    assert!(validate_svg_import_geometry(valid, 0.0, Vec::new(), None).is_err());
}

#[test]
fn svg_import_cancel_rejects_an_applied_token() {
    let state = SvgImportState::default();
    let mut project = initial_project_state();
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let import_id = stage_pending_svg_import(
        &state,
        project.instance_id,
        expected_project_id,
        expected_revision,
        br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.to_vec(),
    )
    .expect("stage SVG import");
    {
        let mut slot = lock_svg_import(&state).expect("lock SVG stage");
        commit_svg_import_replacement(
            &mut project,
            &mut slot.pending,
            import_id,
            expected_project_id,
            expected_revision,
            true,
            create_new_project_state(new_project_parameters()).unwrap(),
        )
        .expect("apply SVG replacement");
    }
    assert!(cancel_pending_svg_import(&state, import_id).is_err());
}
