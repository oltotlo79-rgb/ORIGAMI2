use super::*;

#[test]
fn every_editor_constructor_uses_the_default_history_limit_and_clone_preserves_it() {
    let pattern = CreasePattern::empty();
    let paper = Paper::default();
    let timeline = InstructionTimeline::default();
    let constraints = GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: Vec::new(),
    };

    assert_eq!(
        EditorState::new(pattern.clone()).history_entry_limit(),
        MAX_EDITOR_HISTORY_ENTRIES
    );
    assert_eq!(
        EditorState::with_paper(pattern.clone(), paper.clone()).history_entry_limit(),
        MAX_EDITOR_HISTORY_ENTRIES
    );
    assert_eq!(
        EditorState::with_document_parts(pattern.clone(), paper.clone(), timeline.clone())
            .history_entry_limit(),
        MAX_EDITOR_HISTORY_ENTRIES
    );
    assert_eq!(
        EditorState::with_document_parts_and_constraints(pattern, paper, timeline, constraints,)
            .history_entry_limit(),
        MAX_EDITOR_HISTORY_ENTRIES
    );

    let mut configured = EditorState::new(CreasePattern::empty());
    configured
        .set_history_entry_limit(7)
        .expect("valid history limit");
    assert_eq!(configured.clone().history_entry_limit(), 7);
}

#[test]
fn setting_history_limit_trims_both_stacks_from_the_oldest_side_without_touching_state() {
    let mut editor = EditorState::new(CreasePattern::empty());
    let vertex_ids = (0..6)
        .map(|index| {
            let id = VertexId::new();
            editor
                .execute(
                    editor.revision(),
                    Command::AddVertex {
                        id,
                        position: Point2::new(f64::from(index), 0.0),
                    },
                )
                .expect("add history fixture vertex");
            id
        })
        .collect::<Vec<_>>();
    editor
        .undo(editor.revision())
        .expect("create first redo entry");
    editor
        .undo(editor.revision())
        .expect("create second redo entry");
    let pose = runtime_pose(15.0);
    editor.adopt_current_applied_pose(pose.clone());
    let document_before = (
        editor.pattern.clone(),
        editor.paper.clone(),
        editor.geometric_constraints.clone(),
        editor.instruction_timeline.clone(),
    );
    let revision_before = editor.revision();

    editor
        .set_history_entry_limit(1)
        .expect("minimum history limit is valid");

    assert_eq!(editor.history_entry_limit(), 1);
    assert_eq!(editor.undo_stack.len(), 1);
    assert_eq!(editor.redo_stack.len(), 1);
    assert!(matches!(
        &editor.undo_stack[0].forward,
        Command::AddVertex { id, .. } if *id == vertex_ids[3]
    ));
    assert!(matches!(
        &editor.redo_stack[0].forward,
        Command::AddVertex { id, .. } if *id == vertex_ids[4]
    ));
    assert_eq!(
        (
            editor.pattern.clone(),
            editor.paper.clone(),
            editor.geometric_constraints.clone(),
            editor.instruction_timeline.clone(),
        ),
        document_before
    );
    assert_eq!(editor.revision(), revision_before);
    assert_eq!(editor.current_applied_pose(), Some(&pose));
}

#[test]
fn increasing_history_limit_does_not_restore_trimmed_entries() {
    let mut editor = EditorState::new(CreasePattern::empty());
    editor
        .set_history_entry_limit(2)
        .expect("small history limit");
    let mut vertex_ids = Vec::new();
    for index in 0..5 {
        let id = VertexId::new();
        vertex_ids.push(id);
        editor
            .execute(
                editor.revision(),
                Command::AddVertex {
                    id,
                    position: Point2::new(f64::from(index), 0.0),
                },
            )
            .expect("add history fixture vertex");
    }
    assert_eq!(editor.undo_stack.len(), 2);
    assert!(matches!(
        &editor.undo_stack[0].forward,
        Command::AddVertex { id, .. } if *id == vertex_ids[3]
    ));

    editor
        .set_history_entry_limit(4)
        .expect("increased history limit");
    assert_eq!(editor.undo_stack.len(), 2);
    for index in 5..8 {
        let id = VertexId::new();
        vertex_ids.push(id);
        editor
            .execute(
                editor.revision(),
                Command::AddVertex {
                    id,
                    position: Point2::new(f64::from(index), 0.0),
                },
            )
            .expect("add post-increase history fixture vertex");
    }

    assert_eq!(editor.undo_stack.len(), 4);
    assert!(matches!(
        &editor.undo_stack[0].forward,
        Command::AddVertex { id, .. } if *id == vertex_ids[4]
    ));
    for _ in 0..4 {
        editor
            .undo(editor.revision())
            .expect("undo retained history");
    }
    assert!(!editor.can_undo());
    assert_eq!(editor.redo_stack.len(), 4);
    assert_eq!(
        editor
            .pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>(),
        vertex_ids[..4]
    );
}

#[test]
fn execute_undo_and_redo_pushes_all_use_the_instance_history_limit() {
    let mut editor = EditorState::new(CreasePattern::empty());
    editor
        .set_history_entry_limit(2)
        .expect("small history limit");
    for index in 0..4 {
        editor
            .execute(
                editor.revision(),
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(f64::from(index), 0.0),
                },
            )
            .expect("add history fixture vertex");
    }
    assert_eq!(editor.undo_stack.len(), 2);

    editor.undo(editor.revision()).expect("first undo");
    editor.undo(editor.revision()).expect("second undo");
    assert_eq!(editor.redo_stack.len(), 2);

    editor.redo(editor.revision()).expect("first redo");
    editor.redo(editor.revision()).expect("second redo");
    assert_eq!(editor.undo_stack.len(), 2);
    assert!(!editor.can_redo());
}

#[test]
fn invalid_history_limits_are_atomic_at_both_boundaries() {
    let mut editor = EditorState::new(CreasePattern::empty());
    for index in 0..3 {
        editor
            .execute(
                editor.revision(),
                Command::AddVertex {
                    id: VertexId::new(),
                    position: Point2::new(f64::from(index), 0.0),
                },
            )
            .expect("add history fixture vertex");
    }
    editor.undo(editor.revision()).expect("create redo history");
    editor.adopt_current_applied_pose(runtime_pose(25.0));
    let before = editor_state_snapshot(&editor);

    for requested in [0, MAX_EDITOR_HISTORY_ENTRIES + 1] {
        assert_eq!(
            editor.set_history_entry_limit(requested),
            Err(HistoryEntryLimitError::OutOfRange {
                requested,
                minimum: 1,
                maximum: MAX_EDITOR_HISTORY_ENTRIES,
            })
        );
        assert_eq!(editor_state_snapshot(&editor), before);
    }
}

#[test]
fn history_limit_accepts_both_inclusive_boundaries() {
    let mut editor = EditorState::new(CreasePattern::empty());
    editor
        .set_history_entry_limit(1)
        .expect("minimum history limit");
    assert_eq!(editor.history_entry_limit(), 1);

    editor
        .set_history_entry_limit(MAX_EDITOR_HISTORY_ENTRIES)
        .expect("maximum history limit");
    assert_eq!(editor.history_entry_limit(), MAX_EDITOR_HISTORY_ENTRIES);
}
