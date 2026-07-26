use super::*;

fn horizontal_fixture() -> (
    CreasePattern,
    GeometricConstraintDocumentV1,
    VertexId,
    VertexId,
) {
    let start = VertexId::new();
    let end = VertexId::new();
    let edge = EdgeId::new();
    (
        CreasePattern {
            vertices: vec![
                ori_domain::Vertex {
                    id: start,
                    position: Point2::new(0.0, 0.0),
                },
                ori_domain::Vertex {
                    id: end,
                    position: Point2::new(4.0, 1.0),
                },
            ],
            edges: vec![ori_domain::Edge {
                id: edge,
                start,
                end,
                kind: EdgeKind::Auxiliary,
            }],
        },
        GeometricConstraintDocumentV1 {
            schema_version: ori_core::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Horizontal { edge },
            }],
        },
        start,
        end,
    )
}

fn apply_positions(pattern: &CreasePattern, positions: &[(VertexId, Point2)]) -> CreasePattern {
    let mut candidate = pattern.clone();
    for (id, point) in positions {
        candidate
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == *id)
            .expect("prepared vertex")
            .position = *point;
    }
    candidate
}

#[test]
fn numerical_preview_promotes_only_the_exact_assignment_delta() {
    let (pattern, document, start, _) = horizontal_fixture();
    let solved = solve_geometric_constraints_v1(
        &pattern,
        &document,
        start,
        Point2::new(0.0, 0.0),
        ConstraintSolveLimitsV1::default(),
    )
    .expect("numerical preview");
    let raw_candidate = apply_positions(&pattern, &solved.positions);
    assert!(
        ori_core::certify_binary64_exact_geometric_constraint_satisfaction_v1(
            &raw_candidate,
            &document,
        )
        .expect("valid numerical candidate")
        .is_none(),
    );

    let prepared = prepare_geometric_constraint_solve(&pattern, &document, &solved);
    let exact = prepared
        .exact_satisfaction
        .expect("axis exactification metadata");
    assert_eq!(
        exact.model_id,
        ori_core::GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID_V1,
    );
    assert_eq!(exact.constraint_count, 1);
    assert_eq!(exact.equation_count, 1);
    assert!(!exact.authorizes_project_mutation);
    assert!(!exact.replayable_across_runtimes);
    assert!(prepared.positions.iter().all(|(id, point)| {
        let original = pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == *id)
            .expect("original vertex")
            .position;
        original.x.to_bits() != point.x.to_bits() || original.y.to_bits() != point.y.to_bits()
    }));
    let promoted_candidate = apply_positions(&pattern, &prepared.positions);
    assert!(
        ori_core::certify_binary64_exact_geometric_constraint_satisfaction_v1(
            &promoted_candidate,
            &document,
        )
        .expect("valid promoted candidate")
        .is_some(),
    );

    let expectation = ProjectExpectation::new(ProjectId::new(), ProjectId::new(), 7);
    let (response, stage) = finish_geometric_constraint_solve_preview(
        ProjectId::new(),
        expectation,
        &pattern,
        &document,
        &solved,
        None,
    );
    assert_eq!(stage.project_instance_id, expectation.instance_id);
    assert_eq!(stage.project_id, expectation.project_id);
    assert_eq!(stage.revision, expectation.revision);
    assert_eq!(stage.positions, prepared.positions);
    assert_eq!(stage.exact_satisfaction, prepared.exact_satisfaction);
    assert_eq!(stage.expression_bindings, None);
    let value = serde_json::to_value(response).expect("strict response");
    assert_eq!(
        value["exactSatisfaction"]["modelId"],
        ori_core::GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID_V1,
    );
    assert_eq!(
        value["exactSatisfaction"]["authorizesProjectMutation"],
        false,
    );
    assert_eq!(
        value["exactSatisfaction"]["replayableAcrossRuntimes"],
        false,
    );
}

#[test]
fn shared_finisher_adopts_promoted_exact_positions_for_expression_bindings() {
    let (pattern, document, start, _) = horizontal_fixture();
    let solved = solve_geometric_constraints_v1(
        &pattern,
        &document,
        start,
        Point2::new(0.0, 0.0),
        ConstraintSolveLimitsV1::default(),
    )
    .expect("numerical preview");
    let raw_candidate = apply_positions(&pattern, &solved.positions);
    let (_, plain_stage) = finish_geometric_constraint_solve_preview(
        ProjectId::new(),
        ProjectExpectation::new(ProjectId::new(), ProjectId::new(), 3),
        &pattern,
        &document,
        &solved,
        None,
    );
    let (binding_vertex, exact_point) = plain_stage
        .positions
        .iter()
        .find(|(vertex, exact)| {
            let raw = raw_candidate
                .vertices
                .iter()
                .find(|candidate| candidate.id == *vertex)
                .expect("raw candidate vertex")
                .position;
            raw.x.to_bits() != exact.x.to_bits() || raw.y.to_bits() != exact.y.to_bits()
        })
        .copied()
        .expect("exact projection must differ from the tolerance-only candidate");
    let raw_point = raw_candidate
        .vertices
        .iter()
        .find(|vertex| vertex.id == binding_vertex)
        .expect("raw expression vertex")
        .position;
    let source_binding =
        VertexCoordinateExpressions::new(binding_vertex, "0", "0", raw_point.x, raw_point.y);

    let (_, expression_stage) = finish_geometric_constraint_solve_preview(
        ProjectId::new(),
        ProjectExpectation::new(ProjectId::new(), ProjectId::new(), 3),
        &pattern,
        &document,
        &solved,
        Some(std::slice::from_ref(&source_binding)),
    );
    assert_eq!(expression_stage.positions, plain_stage.positions);
    assert_eq!(
        expression_stage.exact_satisfaction,
        plain_stage.exact_satisfaction,
    );
    let adopted = expression_stage
        .expression_bindings
        .expect("expression stage")[0]
        .clone();
    assert_eq!(adopted.vertex, binding_vertex);
    assert_eq!(adopted.adopted_x_mm.to_bits(), exact_point.x.to_bits());
    assert_eq!(adopted.adopted_y_mm.to_bits(), exact_point.y.to_bits());
    assert!(
        adopted.adopted_x_mm.to_bits() != raw_point.x.to_bits()
            || adopted.adopted_y_mm.to_bits() != raw_point.y.to_bits(),
        "the saved binding must use promoted exact coordinates, not raw solver output",
    );
}

#[test]
fn all_three_native_preview_entries_use_the_shared_exact_finisher() {
    let source = include_str!("geometric_constraint_commands.rs");
    assert_eq!(
        source
            .matches("let (response, stage) = finish_geometric_constraint_solve_preview(")
            .count(),
        3,
        "vertex, edge, and saved-expression previews must share exact staging",
    );
}

#[test]
fn failed_exactification_preserves_the_existing_numerical_preview() {
    let (pattern, _, start, end) = horizontal_fixture();
    let edge = pattern.edges[0].id;
    let document = GeometricConstraintDocumentV1 {
        schema_version: ori_core::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::FixedLength {
                edge,
                length_mm: 3.0,
            },
        }],
    };
    let solved = ori_core::ConstraintSolvePreviewV1 {
        positions: vec![(start, Point2::new(0.0, 0.0)), (end, Point2::new(2.0, 0.0))],
        iterations: 1,
        maximum_residual: 0.0,
        rank: 1,
        degrees_of_freedom: 0,
        equation_count: 1,
        condition_estimate: 1.0,
    };

    let prepared = prepare_geometric_constraint_solve(&pattern, &document, &solved);
    assert_eq!(prepared.positions, solved.positions);
    assert_eq!(prepared.exact_satisfaction, None);
}

#[test]
fn exact_stage_is_recertified_before_apply_and_remains_exact_after_apply() {
    let (pattern, document, start, _) = horizontal_fixture();
    let mut project = ProjectState::new(pattern);
    let project_instance_id = project.instance_id;
    let project_id = project.project_id;
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(project_instance_id, project_id, 0),
        Command::AddGeometricConstraint {
            record: document.constraints[0].clone(),
        },
    )
    .expect("add horizontal constraint");
    let solved = solve_geometric_constraints_v1(
        project.editor.pattern(),
        project.editor.geometric_constraints(),
        start,
        Point2::new(0.0, 0.0),
        ConstraintSolveLimitsV1::default(),
    )
    .expect("numerical preview");
    let prepared = prepare_geometric_constraint_solve(
        project.editor.pattern(),
        project.editor.geometric_constraints(),
        &solved,
    );
    let stage = GeometricConstraintSolveStage {
        token: ProjectId::new(),
        project_instance_id,
        project_id,
        revision: 1,
        positions: prepared.positions,
        expression_bindings: None,
        exact_satisfaction: prepared.exact_satisfaction,
    };
    assert!(stage.exact_satisfaction.is_some());

    let snapshot = apply_geometric_constraint_solve_stage(
        &mut project,
        &stage,
        project_instance_id,
        project_id,
        1,
        stage.token,
    )
    .expect("explicit exact apply");
    assert_eq!(snapshot.revision, 2);
    assert!(
        ori_core::certify_binary64_exact_geometric_constraint_satisfaction_v1(
            project.editor.pattern(),
            project.editor.geometric_constraints(),
        )
        .expect("applied pattern remains valid")
        .is_some(),
    );
}

#[test]
fn already_exact_empty_delta_requires_confirmation_but_creates_no_revision() {
    let (mut pattern, document, start, end) = horizontal_fixture();
    pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == end)
        .expect("end vertex")
        .position
        .y = 0.0;
    let mut project = ProjectState::new(pattern);
    let project_instance_id = project.instance_id;
    let project_id = project.project_id;
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(project_instance_id, project_id, 0),
        Command::AddGeometricConstraint {
            record: document.constraints[0].clone(),
        },
    )
    .expect("add horizontal constraint");
    let solved = solve_geometric_constraints_v1(
        project.editor.pattern(),
        project.editor.geometric_constraints(),
        start,
        Point2::new(0.0, 0.0),
        ConstraintSolveLimitsV1::default(),
    )
    .expect("already exact numerical preview");
    let (_, stage) = finish_geometric_constraint_solve_preview(
        ProjectId::new(),
        ProjectExpectation::new(project_instance_id, project_id, 1),
        project.editor.pattern(),
        project.editor.geometric_constraints(),
        &solved,
        None,
    );
    assert!(stage.positions.is_empty());
    assert!(stage.exact_satisfaction.is_some());

    let snapshot = apply_geometric_constraint_solve_stage(
        &mut project,
        &stage,
        project_instance_id,
        project_id,
        1,
        stage.token,
    )
    .expect("explicit no-op confirmation");
    assert_eq!(snapshot.revision, 1);
    assert_eq!(project.editor.revision(), 1);
}

#[test]
fn one_ulp_stage_tampering_fails_before_project_mutation() {
    let (pattern, document, start, _) = horizontal_fixture();
    let mut project = ProjectState::new(pattern);
    let project_instance_id = project.instance_id;
    let project_id = project.project_id;
    execute_expected_command(
        &mut project,
        ProjectExpectation::new(project_instance_id, project_id, 0),
        Command::AddGeometricConstraint {
            record: document.constraints[0].clone(),
        },
    )
    .expect("add horizontal constraint");
    let solved = solve_geometric_constraints_v1(
        project.editor.pattern(),
        project.editor.geometric_constraints(),
        start,
        Point2::new(0.0, 0.0),
        ConstraintSolveLimitsV1::default(),
    )
    .expect("numerical preview");
    let prepared = prepare_geometric_constraint_solve(
        project.editor.pattern(),
        project.editor.geometric_constraints(),
        &solved,
    );
    let mut stage = GeometricConstraintSolveStage {
        token: ProjectId::new(),
        project_instance_id,
        project_id,
        revision: 1,
        positions: prepared.positions,
        expression_bindings: None,
        exact_satisfaction: prepared.exact_satisfaction,
    };
    let before = project.editor.pattern().clone();
    stage.positions[0].1.y = f64::from_bits(stage.positions[0].1.y.to_bits().wrapping_add(1));

    assert!(
        apply_geometric_constraint_solve_stage(
            &mut project,
            &stage,
            project_instance_id,
            project_id,
            1,
            stage.token,
        )
        .is_err(),
    );
    assert_eq!(project.editor.revision(), 1);
    assert_eq!(project.editor.pattern(), &before);
}
