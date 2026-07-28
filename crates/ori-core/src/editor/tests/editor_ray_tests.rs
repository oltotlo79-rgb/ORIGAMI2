use super::*;

#[test]
fn ray_to_first_edge_splits_and_adds_as_one_history_entry() {
    let source = vertex_at(10.0, 25.0);
    let low = vertex_at(15.0, 24.0);
    let high = vertex_at(15.0, 26.0);
    let target = Edge {
        id: EdgeId::new(),
        start: low.id,
        end: high.id,
        kind: EdgeKind::Valley,
    };
    let sheet = crate::create_rectangular_sheet(100.0, 50.0, false).expect("valid sheet");
    let mut original = sheet.pattern().clone();
    original.vertices.extend([source.clone(), low, high]);
    original.edges.push(target);
    let mut editor = EditorState::with_paper(original.clone(), sheet.paper().clone());
    let command = editor
        .plan_add_ray_to_first_target(0, source.id, 0, EdgeKind::Mountain)
        .expect("plan ray hit");
    editor
        .execute(0, command)
        .expect("apply ray hit atomically");
    assert_eq!(editor.revision(), 1);
    assert_eq!(editor.pattern().vertices.len(), original.vertices.len() + 1);
    assert_eq!(editor.pattern().edges.len(), original.edges.len() + 2);
    let applied = editor.pattern().clone();
    editor.undo(1).expect("undo ray edit");
    assert_eq!(editor.pattern(), &original);
    editor.redo(2).expect("redo ray edit");
    assert_eq!(editor.pattern(), &applied);
}

#[test]
fn ray_prefers_nearest_isolated_vertex_and_rejects_collinear_or_stale() {
    let source = vertex_at(10.0, 25.0);
    let nearest = vertex_at(12.0, 25.0);
    let far = vertex_at(14.0, 25.0);
    let sheet = crate::create_rectangular_sheet(100.0, 50.0, false).expect("valid sheet");
    let mut pattern = sheet.pattern().clone();
    pattern
        .vertices
        .extend([source.clone(), nearest.clone(), far.clone()]);
    let mut editor = EditorState::with_paper(pattern, sheet.paper().clone());
    let command = editor
        .plan_add_ray_to_first_target(0, source.id, 0, EdgeKind::Auxiliary)
        .expect("isolated vertex is a ray target");
    editor.execute(0, command).expect("add to nearest vertex");
    assert!(
        editor
            .pattern()
            .edges
            .iter()
            .any(|edge| { edge.start == source.id && edge.end == nearest.id })
    );
    assert_eq!(
        editor.plan_add_ray_to_first_target(0, source.id, 0, EdgeKind::Auxiliary,),
        Err(CommandError::RevisionConflict {
            expected: 0,
            actual: 1
        })
    );

    let collinear = Edge {
        id: EdgeId::new(),
        start: nearest.id,
        end: far.id,
        kind: EdgeKind::Valley,
    };
    let editor = EditorState::new(CreasePattern {
        vertices: vec![source.clone(), nearest, far],
        edges: vec![collinear],
    });
    assert_eq!(
        editor.plan_add_ray_to_first_target(0, source.id, 0, EdgeKind::Mountain,),
        Err(CommandError::RayTargetAmbiguous)
    );
}

#[test]
fn ray_rejects_invalid_angle_missing_target_and_invalid_paper_without_mutation() {
    let source = vertex_at(0.0, 0.0);
    let mut editor = EditorState::new(CreasePattern {
        vertices: vec![source.clone()],
        edges: Vec::new(),
    });
    let paper = editor.paper().clone();
    assert_eq!(
        editor.plan_add_ray_to_first_target(0, source.id, 360_000_000, EdgeKind::Valley),
        Err(CommandError::InvalidRayAngle)
    );
    assert_eq!(
        editor.plan_add_ray_to_first_target(0, source.id, 90_000_000, EdgeKind::Valley),
        Err(CommandError::RayTargetNotFound)
    );
    let target = vertex_at(1.0, 0.0);
    editor.pattern.vertices.push(target);
    let before_command = editor.pattern().clone();
    let command = editor
        .plan_add_ray_to_first_target(0, source.id, 0, EdgeKind::Valley)
        .expect("plan on legacy fixture");
    assert_eq!(
        editor.execute(0, command),
        Err(CommandError::InvalidStackedFoldDocument)
    );
    assert_eq!(editor.pattern(), &before_command);
    assert_eq!(editor.paper(), &paper);
    assert_eq!(editor.revision(), 0);
}

#[test]
fn ray_splits_boundary_and_updates_paper_order_atomically() {
    let sheet = crate::create_rectangular_sheet(100.0, 50.0, false).expect("valid sheet");
    let mut pattern = sheet.pattern().clone();
    let source = vertex_at(10.0, 25.0);
    pattern.vertices.push(source.clone());
    let mut editor = EditorState::with_paper(pattern, sheet.paper().clone());
    let before_pattern = editor.pattern().clone();
    let before_paper = editor.paper().clone();
    let command = editor
        .plan_add_ray_to_first_target(0, source.id, 180_000_000, EdgeKind::Mountain)
        .expect("boundary hit");
    editor.execute(0, command).expect("atomic boundary split");
    assert_eq!(
        editor.paper().boundary_vertices.len(),
        before_paper.boundary_vertices.len() + 1
    );
    assert_eq!(
        editor.pattern().vertices.len(),
        before_pattern.vertices.len() + 1
    );
    assert_eq!(editor.pattern().edges.len(), before_pattern.edges.len() + 2);
    let inserted = editor
        .paper()
        .boundary_vertices
        .iter()
        .copied()
        .find(|id| !before_paper.boundary_vertices.contains(id))
        .expect("inserted boundary vertex");
    let inserted_vertex = editor
        .pattern()
        .vertices
        .iter()
        .find(|v| v.id == inserted)
        .expect("inserted geometry");
    assert_eq!(inserted_vertex.position, Point2 { x: 0.0, y: 25.0 });
    let index = editor
        .paper()
        .boundary_vertices
        .iter()
        .position(|id| *id == inserted)
        .unwrap();
    let boundary = &editor.paper().boundary_vertices;
    let previous = boundary[(index + boundary.len() - 1) % boundary.len()];
    let next = boundary[(index + 1) % boundary.len()];
    let position = |id| {
        editor
            .pattern()
            .vertices
            .iter()
            .find(|v| v.id == id)
            .unwrap()
            .position
    };
    assert_eq!(position(previous).x, 0.0);
    assert_eq!(position(next).x, 0.0);
    assert_eq!(
        editor
            .pattern()
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Boundary && (e.start == inserted || e.end == inserted))
            .count(),
        2
    );
    assert_eq!(
        editor
            .pattern()
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Mountain
                && ((e.start == source.id && e.end == inserted)
                    || (e.end == source.id && e.start == inserted)))
            .count(),
        1
    );
    let applied_pattern = editor.pattern().clone();
    let applied_paper = editor.paper().clone();
    editor.undo(1).expect("undo boundary ray");
    assert_eq!(editor.pattern(), &before_pattern);
    assert_eq!(editor.paper(), &before_paper);
    editor.redo(2).expect("redo boundary ray");
    assert_eq!(editor.pattern(), &applied_pattern);
    assert_eq!(editor.paper(), &applied_paper);
    let history = editor
        .export_history_v1(ori_domain::ProjectId::new())
        .expect("export ray history");
    let mut reopened = EditorState::with_document_parts_layers_and_history_v1(
        editor.pattern().clone(),
        editor.paper().clone(),
        editor.instruction_timeline().clone(),
        editor.geometric_constraints().clone(),
        editor.project_layers().clone(),
        history,
    )
    .expect("reopen authenticated ray history");
    reopened.undo(0).expect("undo reopened ray");
    assert_eq!(reopened.pattern(), &before_pattern);
    assert_eq!(reopened.paper(), &before_paper);
    reopened.redo(1).expect("redo reopened ray");
    assert_eq!(reopened.pattern(), &applied_pattern);
    assert_eq!(reopened.paper(), &applied_paper);
}

#[test]
fn ray_kernel_handles_tiny_scale_cardinal_and_upper_angle_and_rejects_near_tie() {
    let source = vertex_at(0.0, 0.0);
    let north = vertex_at(0.0, 1.0e-12);
    let editor = EditorState::new(CreasePattern {
        vertices: vec![source.clone(), north],
        edges: Vec::new(),
    });
    assert!(
        editor
            .plan_add_ray_to_first_target(0, source.id, 90_000_000, EdgeKind::Auxiliary)
            .is_ok()
    );

    let (sine, cosine) = ori_numeric::deterministic_sin_cos_degrees_v1(359.999_999)
        .expect("deterministic upper angle");
    let upper = vertex_at(1.0e-12 * cosine, 1.0e-12 * sine);
    let editor = EditorState::new(CreasePattern {
        vertices: vec![source.clone(), upper],
        edges: Vec::new(),
    });
    assert!(
        editor
            .plan_add_ray_to_first_target(0, source.id, 359_999_999, EdgeKind::Valley)
            .is_ok()
    );

    let first = vertex_at(1.0, 0.0);
    let tied = vertex_at(1.0 + 64.0 * f64::EPSILON, 0.0);
    let editor = EditorState::new(CreasePattern {
        vertices: vec![source.clone(), first, tied],
        edges: Vec::new(),
    });
    assert_eq!(
        editor.plan_add_ray_to_first_target(0, source.id, 0, EdgeKind::Mountain),
        Err(CommandError::RayTargetAmbiguous)
    );

    let junction = vertex_at(1.0, 0.0);
    let non_finite = vertex_at(f64::NAN, 1.0);
    let hidden_incident = Edge {
        id: EdgeId::new(),
        start: junction.id,
        end: non_finite.id,
        kind: EdgeKind::Valley,
    };
    let editor = EditorState::new(CreasePattern {
        vertices: vec![source.clone(), junction, non_finite],
        edges: vec![hidden_incident],
    });
    assert_eq!(
        editor.plan_add_ray_to_first_target(0, source.id, 0, EdgeKind::Mountain),
        Err(CommandError::RayTargetAmbiguous)
    );

    let sheet = crate::create_rectangular_sheet(100.0, 50.0, false).expect("valid sheet");
    let corner = sheet
        .paper()
        .boundary_vertices
        .iter()
        .copied()
        .find(|id| {
            sheet
                .pattern()
                .vertices
                .iter()
                .find(|v| v.id == *id)
                .is_some_and(|v| v.position == Point2 { x: 0.0, y: 0.0 })
        })
        .expect("lower-left boundary source");
    let editor = EditorState::with_paper(sheet.pattern().clone(), sheet.paper().clone());
    assert_eq!(
        editor.plan_add_ray_to_first_target(0, corner, 225_000_000, EdgeKind::Auxiliary),
        Err(CommandError::RayTargetNotFound)
    );
}

#[test]
fn ray_sealed_plan_rejects_semantic_tampering_without_mutation() {
    let sheet = crate::create_rectangular_sheet(100.0, 50.0, false).expect("valid sheet");
    let mut pattern = sheet.pattern().clone();
    let source = vertex_at(10.0, 25.0);
    pattern.vertices.push(source.clone());
    let editor = EditorState::with_paper(pattern, sheet.paper().clone());
    let command = editor
        .plan_add_ray_to_first_target(0, source.id, 180_000_000, EdgeKind::Mountain)
        .expect("sealed plan");
    let verify_rejected = |command: Command| {
        let mut candidate = editor.clone();
        let before_pattern = candidate.pattern().clone();
        let before_paper = candidate.paper().clone();
        let before_layers = candidate.project_layers().clone();
        assert_eq!(
            candidate.execute(0, command),
            Err(CommandError::InvalidStackedFoldDocument)
        );
        assert_eq!(candidate.pattern(), &before_pattern);
        assert_eq!(candidate.paper(), &before_paper);
        assert_eq!(candidate.project_layers(), &before_layers);
        assert_eq!(candidate.revision(), 0);
    };
    let mut angle = command.clone();
    let Command::ApplyRayToTargetDocument(plan) = &mut angle else {
        panic!("ray plan")
    };
    plan.angle_microdegrees = 360_000_000;
    verify_rejected(angle);
    let mut kind = command.clone();
    let Command::ApplyRayToTargetDocument(plan) = &mut kind else {
        panic!("ray plan")
    };
    plan.kind = EdgeKind::Boundary;
    verify_rejected(kind);
    let mut changed = command.clone();
    let Command::ApplyRayToTargetDocument(plan) = &mut changed else {
        panic!("ray plan")
    };
    plan.changed_edges.push(EdgeId::new());
    verify_rejected(changed);
    let mut paper = command.clone();
    let Command::ApplyRayToTargetDocument(plan) = &mut paper else {
        panic!("ray plan")
    };
    plan.paper.thickness_mm += 1.0;
    verify_rejected(paper);
    let mut layers = command;
    let Command::ApplyRayToTargetDocument(plan) = &mut layers else {
        panic!("ray plan")
    };
    plan.project_layers.layers[0].name.push_str(" tampered");
    verify_rejected(layers);
}
