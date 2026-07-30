use super::*;

fn beginner_profile_with_test_generation_provenance() -> BeginnerDesignProfileV1 {
    let mut profile = BeginnerDesignProfileV1::default();
    profile.generation_provenance = Some(ori_domain::BeginnerGenerationProvenanceV1 {
        schema_version: 1,
        topology_authority_sha256: [17; 32],
        fold_path_certificate_sha256: Some([19; 32]),
        document_authority_sha256: None,
        confidence_score: 90,
        confidence_reasons: vec!["core_history_test_v1".to_owned()],
        explicit_override: false,
        source_asset_fingerprint: "core-history-test-source-v1".to_owned(),
        semantic_landmark_provenance: None,
        generic_tree: None,
        reference_consensus: None,
        reference_consensus_summary: None,
    });
    assert!(validate_beginner_design_profile_v1(&profile));
    profile
}

fn mint_test_generation_provenance(editor: &mut EditorState, profile: BeginnerDesignProfileV1) {
    let mut instruction_timeline = editor.instruction_timeline().clone();
    instruction_timeline.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: "Generated authority".to_owned(),
        description: "History authority anchor".to_owned(),
        caution: "Test-only declarative instruction".to_owned(),
        duration_ms: 1_000,
        visual: InstructionVisual::default(),
        pose: InstructionPose {
            model: ori_domain::InstructionPoseModel::DeclarativeOnlyV1,
            source_model_fingerprint: editor.fold_model_fingerprint_v1(),
            fixed_face: None,
            hinge_angles: Vec::new(),
        },
    });
    editor
        .execute(
            editor.revision(),
            Command::ApplyBeginnerGeneratedDocument {
                pattern: editor.pattern().clone(),
                paper: editor.paper().clone(),
                instruction_timeline,
                project_layers: editor.project_layers().clone(),
                beginner_design_profile: Box::new(profile),
            },
        )
        .expect("mint generation provenance through an authorized history edge");
}

#[test]
fn beginner_generation_provenance_invalidates_with_geometry_and_round_trips_exactly() {
    let (mut editor, original_pattern, _) = simple_rectangular_editor();
    let proven_profile = beginner_profile_with_test_generation_provenance();
    editor
        .restore_beginner_design_profile(proven_profile.clone())
        .unwrap();
    let source = original_pattern.vertices[0].clone();
    let moved = Point2::new(source.position.x + 1.0, source.position.y + 1.0);

    editor
        .execute(
            0,
            Command::MoveVertex {
                id: source.id,
                position: moved,
            },
        )
        .unwrap();
    let moved_pattern = editor.pattern().clone();
    assert!(
        editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );

    editor.undo(1).unwrap();
    assert_eq!(editor.pattern(), &original_pattern);
    assert_eq!(editor.beginner_design_profile(), &proven_profile);

    editor.redo(2).unwrap();
    assert_eq!(editor.pattern(), &moved_pattern);
    assert!(
        editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );
}

#[test]
fn beginner_generation_provenance_survives_noop_failure_stale_and_presentation_only_edits() {
    let (mut editor, pattern, _) = simple_rectangular_editor();
    let proven_profile = beginner_profile_with_test_generation_provenance();
    editor
        .restore_beginner_design_profile(proven_profile.clone())
        .unwrap();
    let source = pattern.vertices[0].clone();

    editor
        .execute(
            editor.revision(),
            Command::MoveVertex {
                id: source.id,
                position: source.position,
            },
        )
        .unwrap();
    assert_eq!(editor.beginner_design_profile(), &proven_profile);

    let stale_before = editor_state_snapshot(&editor);
    assert_eq!(
        editor.execute(
            0,
            Command::MoveVertex {
                id: source.id,
                position: Point2::new(source.position.x + 1.0, source.position.y + 1.0),
            },
        ),
        Err(CommandError::RevisionConflict {
            expected: 0,
            actual: 1,
        })
    );
    assert_eq!(editor_state_snapshot(&editor), stale_before);

    editor
        .set_history_entry_limit(1)
        .expect("fill the one-entry history limit");
    let full_history_before = editor_state_snapshot(&editor);
    assert_eq!(
        editor.execute(
            1,
            Command::AddVertex {
                id: source.id,
                position: source.position,
            },
        ),
        Err(CommandError::VertexAlreadyExists(source.id))
    );
    assert_eq!(editor_state_snapshot(&editor), full_history_before);
    assert_eq!(
        editor.set_history_entry_limit(0),
        Err(HistoryEntryLimitError::OutOfRange {
            requested: 0,
            minimum: 1,
            maximum: MAX_EDITOR_HISTORY_ENTRIES,
        })
    );
    assert_eq!(editor_state_snapshot(&editor), full_history_before);
    assert_eq!(
        editor.set_history_entry_limit(MAX_EDITOR_HISTORY_ENTRIES + 1),
        Err(HistoryEntryLimitError::OutOfRange {
            requested: MAX_EDITOR_HISTORY_ENTRIES + 1,
            minimum: 1,
            maximum: MAX_EDITOR_HISTORY_ENTRIES,
        })
    );
    assert_eq!(editor_state_snapshot(&editor), full_history_before);

    let presentation = editor.project_layers().layers[0].clone();
    editor
        .execute(
            1,
            Command::UpdateLayerPresentation {
                layer: presentation.id,
                visible: presentation.visible,
                locked: presentation.locked,
                opacity: 0.75,
            },
        )
        .unwrap();
    assert_eq!(editor.beginner_design_profile(), &proven_profile);
}

#[test]
fn beginner_generation_provenance_invalidates_when_profile_authority_changes() {
    let (mut editor, _, _) = simple_rectangular_editor();
    let proven_profile = beginner_profile_with_test_generation_provenance();
    editor
        .restore_beginner_design_profile(proven_profile.clone())
        .unwrap();
    let mut changed = proven_profile.clone();
    changed.generation_constraints.maximum_steps += 1;

    editor
        .execute(
            0,
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(changed.clone()),
            },
        )
        .unwrap();
    changed.generation_provenance = None;
    assert_eq!(editor.beginner_design_profile(), &changed);

    editor.undo(1).unwrap();
    assert_eq!(editor.beginner_design_profile(), &proven_profile);
    editor.redo(2).unwrap();
    assert_eq!(editor.beginner_design_profile(), &changed);

    let (mut reranked_editor, _, _) = simple_rectangular_editor();
    let mut reranked = proven_profile.clone();
    reranked.shape_fidelity_weight += 1;
    reranked.foldability_weight -= 1;
    reranked_editor
        .restore_beginner_design_profile(proven_profile.clone())
        .unwrap();
    reranked_editor
        .execute(
            0,
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(reranked.clone()),
            },
        )
        .unwrap();
    reranked.generation_provenance = None;
    assert_eq!(reranked_editor.beginner_design_profile(), &reranked);
    reranked_editor.undo(1).unwrap();
    assert_eq!(reranked_editor.beginner_design_profile(), &proven_profile);
    reranked_editor.redo(2).unwrap();
    assert_eq!(reranked_editor.beginner_design_profile(), &reranked);
}

#[test]
fn ordinary_profile_updates_reject_authority_escalation_but_allow_explicit_downgrade() {
    let (mut editor, _, _) = simple_rectangular_editor();
    let proven_profile = beginner_profile_with_test_generation_provenance();

    let before_mint = editor_state_snapshot(&editor);
    assert_eq!(
        editor.execute(
            editor.revision(),
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(proven_profile.clone()),
            },
        ),
        Err(CommandError::InvalidBeginnerDesignProfile)
    );
    assert_eq!(editor_state_snapshot(&editor), before_mint);

    editor
        .restore_beginner_design_profile(proven_profile.clone())
        .unwrap();
    let mut tampered = proven_profile.clone();
    tampered
        .generation_provenance
        .as_mut()
        .unwrap()
        .topology_authority_sha256[0] ^= 0xff;
    let before_replace = editor_state_snapshot(&editor);
    assert_eq!(
        editor.execute(
            editor.revision(),
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(tampered),
            },
        ),
        Err(CommandError::InvalidBeginnerDesignProfile)
    );
    assert_eq!(editor_state_snapshot(&editor), before_replace);

    editor
        .execute(
            editor.revision(),
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(proven_profile.clone()),
            },
        )
        .expect("an exact profile no-op preserves existing provenance");
    assert_eq!(editor.beginner_design_profile(), &proven_profile);
    editor.undo(editor.revision()).unwrap();
    assert_eq!(editor.beginner_design_profile(), &proven_profile);
    editor.redo(editor.revision()).unwrap();
    assert_eq!(editor.beginner_design_profile(), &proven_profile);

    let mut cleared = proven_profile.clone();
    cleared.generation_provenance = None;
    editor
        .execute(
            editor.revision(),
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(cleared.clone()),
            },
        )
        .expect("dropping provenance is an explicit safe downgrade");
    assert_eq!(editor.beginner_design_profile(), &cleared);
    editor.undo(editor.revision()).unwrap();
    assert_eq!(editor.beginner_design_profile(), &proven_profile);
    editor.redo(editor.revision()).unwrap();
    assert_eq!(editor.beginner_design_profile(), &cleared);
}

#[test]
fn beginner_generation_layer_assignment_authority_handles_noop_and_layer_deletion() {
    let (mut editor, pattern, _) = simple_rectangular_editor();
    let authored_layer = LayerRecordV1 {
        id: LayerId::new(),
        name: "Authored details".to_owned(),
        content_kind: ori_domain::LayerContentKindV1::CreasePattern,
        visible: true,
        locked: false,
        opacity: 1.0,
    };
    editor
        .execute(
            editor.revision(),
            Command::CreateLayer {
                layer: authored_layer.clone(),
                target_index: 1,
            },
        )
        .unwrap();
    let assigned_edge = pattern.edges[0].id;
    editor
        .execute(
            editor.revision(),
            Command::AssignEdgeToLayer {
                edge: assigned_edge,
                layer: authored_layer.id,
            },
        )
        .unwrap();
    let proven_profile = beginner_profile_with_test_generation_provenance();
    editor
        .restore_beginner_design_profile(proven_profile.clone())
        .unwrap();

    editor
        .execute(
            editor.revision(),
            Command::AssignEdgeToLayer {
                edge: assigned_edge,
                layer: authored_layer.id,
            },
        )
        .expect("same assignment is a provenance-preserving no-op");
    assert_eq!(editor.beginner_design_profile(), &proven_profile);

    editor
        .execute(
            editor.revision(),
            Command::DeleteLayer {
                layer: authored_layer.id,
            },
        )
        .expect("deleting an assigned layer rebinds its edges to default");
    assert_eq!(
        editor.project_layers().layer_for_edge(assigned_edge),
        DEFAULT_PROJECT_LAYER_ID
    );
    assert!(
        editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );

    editor.undo(editor.revision()).unwrap();
    assert_eq!(
        editor.project_layers().layer_for_edge(assigned_edge),
        authored_layer.id
    );
    assert_eq!(editor.beginner_design_profile(), &proven_profile);
    editor.redo(editor.revision()).unwrap();
    assert_eq!(
        editor.project_layers().layer_for_edge(assigned_edge),
        DEFAULT_PROJECT_LAYER_ID
    );
    assert!(
        editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );
}

#[test]
fn beginner_generation_paper_authority_ignores_appearance_but_tracks_thickness_and_boundary() {
    let (mut thickness_editor, _, _) = simple_rectangular_editor();
    let proven_profile = beginner_profile_with_test_generation_provenance();
    thickness_editor
        .restore_beginner_design_profile(proven_profile.clone())
        .unwrap();
    let original_paper = thickness_editor.paper().clone();
    let changed_front_color = if original_paper.front.color == RgbaColor::opaque(12, 34, 56) {
        RgbaColor::opaque(65, 43, 21)
    } else {
        RgbaColor::opaque(12, 34, 56)
    };
    thickness_editor
        .execute(
            thickness_editor.revision(),
            Command::UpdatePaperProperties {
                thickness_mm: original_paper.thickness_mm,
                front_color: changed_front_color,
                back_color: original_paper.back.color,
                front_texture_asset: original_paper.front.texture_asset,
                back_texture_asset: original_paper.back.texture_asset,
                cutting_allowed: original_paper.cutting_allowed,
            },
        )
        .expect("paper appearance is outside generation authority");
    assert_eq!(thickness_editor.beginner_design_profile(), &proven_profile);

    let appearance_paper = thickness_editor.paper().clone();
    thickness_editor
        .execute(
            thickness_editor.revision(),
            Command::UpdatePaperProperties {
                thickness_mm: appearance_paper.thickness_mm + 0.1,
                front_color: appearance_paper.front.color,
                back_color: appearance_paper.back.color,
                front_texture_asset: appearance_paper.front.texture_asset,
                back_texture_asset: appearance_paper.back.texture_asset,
                cutting_allowed: appearance_paper.cutting_allowed,
            },
        )
        .expect("paper thickness changes generation authority");
    assert!(
        thickness_editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );
    thickness_editor.undo(thickness_editor.revision()).unwrap();
    assert_eq!(thickness_editor.paper(), &appearance_paper);
    assert_eq!(thickness_editor.beginner_design_profile(), &proven_profile);
    thickness_editor.redo(thickness_editor.revision()).unwrap();
    assert!(
        thickness_editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );

    let (mut boundary_editor, _, _) = simple_rectangular_editor();
    boundary_editor
        .restore_beginner_design_profile(proven_profile.clone())
        .unwrap();
    let original_boundary_paper = boundary_editor.paper().clone();
    boundary_editor
        .execute(
            boundary_editor.revision(),
            Command::ResizeRectangularPaper {
                width_mm: 120.0,
                height_mm: 60.0,
            },
        )
        .expect("resize changes boundary authority");
    assert!(
        boundary_editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );
    boundary_editor.undo(boundary_editor.revision()).unwrap();
    assert_eq!(boundary_editor.paper(), &original_boundary_paper);
    assert_eq!(boundary_editor.beginner_design_profile(), &proven_profile);
    boundary_editor.redo(boundary_editor.revision()).unwrap();
    assert!(
        boundary_editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );
}

#[test]
fn referenced_underlay_asset_changes_invalidate_but_presentation_and_unrelated_edits_do_not() {
    let (mut editor, _, _) = simple_rectangular_editor();
    let underlay_layer = LayerRecordV1 {
        id: LayerId::new(),
        name: "Reference".to_owned(),
        content_kind: ori_domain::LayerContentKindV1::Underlay,
        visible: true,
        locked: false,
        opacity: 1.0,
    };
    editor
        .execute(
            0,
            Command::CreateLayer {
                layer: underlay_layer.clone(),
                target_index: 1,
            },
        )
        .unwrap();
    let target_underlay_id = UnderlayId::new();
    let target_asset_id = AssetId::new();
    let mut target = UnderlayRecordV1 {
        id: target_underlay_id,
        asset: target_asset_id,
        transform: ori_domain::UnderlayTransformV1 {
            position: Point2::new(0.0, 0.0),
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_degrees: 0.0,
        },
        opacity: 0.5,
        layer: underlay_layer.id,
    };
    editor
        .execute(
            1,
            Command::AddUnderlay {
                record: target.clone(),
            },
        )
        .unwrap();
    let mut proven_profile = beginner_profile_with_test_generation_provenance();
    proven_profile.generation_constraints.target_asset =
        Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceImage {
            underlay_id: target_underlay_id,
            asset_id: target_asset_id,
        });
    assert!(validate_beginner_design_profile_v1(&proven_profile));
    editor = EditorState::with_all_document_parts_annotations_underlays_and_memo(
        editor.pattern().clone(),
        editor.paper().clone(),
        editor.instruction_timeline().clone(),
        editor.geometric_constraints().clone(),
        editor.project_layers().clone(),
        editor.element_metadata().clone(),
        editor.annotations().clone(),
        editor.underlays().clone(),
        editor.project_memo().to_owned(),
    );
    mint_test_generation_provenance(&mut editor, proven_profile.clone());

    let unrelated = UnderlayRecordV1 {
        id: UnderlayId::new(),
        asset: AssetId::new(),
        ..target.clone()
    };
    editor
        .execute(
            editor.revision(),
            Command::AddUnderlay { record: unrelated },
        )
        .unwrap();
    assert_eq!(editor.beginner_design_profile(), &proven_profile);

    target.opacity = 0.75;
    target.transform.position = Point2::new(5.0, 7.0);
    editor
        .execute(
            editor.revision(),
            Command::UpdateUnderlay {
                record: target.clone(),
            },
        )
        .unwrap();
    assert_eq!(editor.beginner_design_profile(), &proven_profile);

    let bound_target_before_asset_change = target.clone();
    target.asset = AssetId::new();
    editor
        .execute(
            editor.revision(),
            Command::UpdateUnderlay {
                record: target.clone(),
            },
        )
        .unwrap();
    assert!(
        editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );
    editor.undo(editor.revision()).unwrap();
    assert_eq!(editor.beginner_design_profile(), &proven_profile);
    assert_eq!(
        editor
            .underlays()
            .underlays
            .iter()
            .find(|record| record.id == target_underlay_id),
        Some(&bound_target_before_asset_change)
    );
    editor.redo(editor.revision()).unwrap();
    assert!(
        editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );

    editor.undo(editor.revision()).unwrap();
    editor
        .execute(
            editor.revision(),
            Command::RemoveUnderlay {
                id: target_underlay_id,
            },
        )
        .unwrap();
    assert!(
        editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );
    editor.undo(editor.revision()).unwrap();
    assert_eq!(editor.beginner_design_profile(), &proven_profile);
    assert_eq!(
        editor
            .underlays()
            .underlays
            .iter()
            .find(|record| record.id == target_underlay_id),
        Some(&bound_target_before_asset_change)
    );
    editor.redo(editor.revision()).unwrap();
    assert!(
        editor
            .underlays()
            .underlays
            .iter()
            .all(|record| record.id != target_underlay_id)
    );
    assert!(
        editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );

    let project_id = ProjectId::new();
    let history = editor.export_history_v1(project_id).unwrap();
    let invalidated_profile = editor.beginner_design_profile().clone();
    let mut reopened =
        EditorState::with_all_document_parts_annotations_underlays_memo_profile_and_history_v1(
            editor.pattern().clone(),
            editor.paper().clone(),
            editor.instruction_timeline().clone(),
            editor.geometric_constraints().clone(),
            editor.project_layers().clone(),
            editor.element_metadata().clone(),
            editor.annotations().clone(),
            editor.underlays().clone(),
            editor.project_memo().to_owned(),
            invalidated_profile.clone(),
            history,
        )
        .unwrap();
    reopened.undo(0).unwrap();
    assert_eq!(reopened.beginner_design_profile(), &proven_profile);
    assert_eq!(
        reopened
            .underlays()
            .underlays
            .iter()
            .find(|record| record.id == target_underlay_id),
        Some(&bound_target_before_asset_change)
    );
    reopened.redo(1).unwrap();
    assert_eq!(reopened.beginner_design_profile(), &invalidated_profile);
    assert!(
        reopened
            .underlays()
            .underlays
            .iter()
            .all(|record| record.id != target_underlay_id)
    );
}

#[test]
fn beginner_generated_document_keeps_its_new_generation_provenance() {
    let (mut editor, pattern, paper) = simple_rectangular_editor();
    let proven_profile = beginner_profile_with_test_generation_provenance();
    let mut instruction_timeline = editor.instruction_timeline().clone();
    instruction_timeline.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: "Generated beginner plan".to_owned(),
        description: "A generated instruction".to_owned(),
        caution: "Test-only declarative instruction".to_owned(),
        duration_ms: 1_000,
        visual: InstructionVisual::default(),
        pose: InstructionPose {
            model: ori_domain::InstructionPoseModel::DeclarativeOnlyV1,
            source_model_fingerprint: editor.fold_model_fingerprint_v1(),
            fixed_face: None,
            hinge_angles: Vec::new(),
        },
    });
    let project_layers = editor.project_layers().clone();

    let mut stacked_editor = editor.clone();
    let stacked_before = editor_state_snapshot(&stacked_editor);
    assert_eq!(
        stacked_editor.execute(
            stacked_editor.revision(),
            Command::ApplyStackedFoldDocument(StackedFoldDocumentCommandV1::new(
                pattern.clone(),
                paper.clone(),
                instruction_timeline.clone(),
                project_layers.clone(),
                Box::new(proven_profile.clone()),
            )),
        ),
        Err(CommandError::InvalidStackedFoldDocument)
    );
    assert_eq!(editor_state_snapshot(&stacked_editor), stacked_before);

    editor
        .execute(
            0,
            Command::ApplyBeginnerGeneratedDocument {
                pattern,
                paper,
                instruction_timeline,
                project_layers,
                beginner_design_profile: Box::new(proven_profile.clone()),
            },
        )
        .unwrap();
    assert_eq!(editor.beginner_design_profile(), &proven_profile);

    editor.undo(1).unwrap();
    assert!(
        editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );
    editor.redo(2).unwrap();
    assert_eq!(editor.beginner_design_profile(), &proven_profile);
}

#[test]
fn beginner_generated_document_rejects_missing_reference_image_authority() {
    let (mut editor, pattern, paper) = simple_rectangular_editor();
    let mut profile = beginner_profile_with_test_generation_provenance();
    profile.generation_constraints.target_asset =
        Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceImage {
            underlay_id: UnderlayId::new(),
            asset_id: AssetId::new(),
        });
    let mut instruction_timeline = editor.instruction_timeline().clone();
    instruction_timeline.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: "Unbound generated authority".to_owned(),
        description: "Must not be admitted".to_owned(),
        caution: "Test-only declarative instruction".to_owned(),
        duration_ms: 1_000,
        visual: InstructionVisual::default(),
        pose: InstructionPose {
            model: ori_domain::InstructionPoseModel::DeclarativeOnlyV1,
            source_model_fingerprint: editor.fold_model_fingerprint_v1(),
            fixed_face: None,
            hinge_angles: Vec::new(),
        },
    });
    let before = editor_state_snapshot(&editor);

    assert_eq!(
        editor.execute(
            editor.revision(),
            Command::ApplyBeginnerGeneratedDocument {
                pattern,
                paper,
                instruction_timeline,
                project_layers: editor.project_layers().clone(),
                beginner_design_profile: Box::new(profile),
            },
        ),
        Err(CommandError::InvalidStackedFoldDocument)
    );
    assert_eq!(editor_state_snapshot(&editor), before);
}

#[test]
fn persisted_provenance_invalidation_reopens_with_exact_undo_and_redo() {
    let (mut editor, original_pattern, _) = simple_rectangular_editor();
    let proven_profile = beginner_profile_with_test_generation_provenance();
    mint_test_generation_provenance(&mut editor, proven_profile.clone());
    let source = original_pattern.vertices[0].clone();
    editor
        .execute(
            editor.revision(),
            Command::MoveVertex {
                id: source.id,
                position: Point2::new(source.position.x + 1.0, source.position.y + 1.0),
            },
        )
        .unwrap();
    let moved_pattern = editor.pattern().clone();
    let invalidated_profile = editor.beginner_design_profile().clone();
    let project_id = ProjectId::new();
    let history = editor.export_history_v1(project_id).unwrap();
    let mut reopened =
        EditorState::with_all_document_parts_annotations_underlays_memo_profile_and_history_v1(
            editor.pattern().clone(),
            editor.paper().clone(),
            editor.instruction_timeline().clone(),
            editor.geometric_constraints().clone(),
            editor.project_layers().clone(),
            editor.element_metadata().clone(),
            editor.annotations().clone(),
            editor.underlays().clone(),
            editor.project_memo().to_owned(),
            invalidated_profile.clone(),
            history,
        )
        .unwrap();

    reopened.undo(0).unwrap();
    assert_eq!(reopened.pattern(), &original_pattern);
    assert_eq!(reopened.beginner_design_profile(), &proven_profile);
    reopened.redo(1).unwrap();
    assert_eq!(reopened.pattern(), &moved_pattern);
    assert_eq!(reopened.beginner_design_profile(), &invalidated_profile);

    reopened.undo(2).unwrap();
    let redo_history = reopened.export_history_v1(project_id).unwrap();
    let mut reopened_redo =
        EditorState::with_all_document_parts_annotations_underlays_memo_profile_and_history_v1(
            reopened.pattern().clone(),
            reopened.paper().clone(),
            reopened.instruction_timeline().clone(),
            reopened.geometric_constraints().clone(),
            reopened.project_layers().clone(),
            reopened.element_metadata().clone(),
            reopened.annotations().clone(),
            reopened.underlays().clone(),
            reopened.project_memo().to_owned(),
            proven_profile.clone(),
            redo_history,
        )
        .unwrap();
    reopened_redo.redo(0).unwrap();
    assert_eq!(reopened_redo.pattern(), &moved_pattern);
    assert_eq!(
        reopened_redo.beginner_design_profile(),
        &invalidated_profile
    );
}
