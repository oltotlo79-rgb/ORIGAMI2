use ori_domain::{
    ConstraintId, CreasePattern, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};

use super::*;

const ROTATION_SCALE: f64 = 1_099_511_627_776.0;

fn rotation_fixture(
    target: Point2,
) -> (
    CreasePattern,
    GeometricConstraintDocumentV1,
    VertexId,
    ConstraintId,
) {
    let center_vertex = VertexId::new();
    let source_vertex = VertexId::new();
    let target_vertex = VertexId::new();
    let constraint = ConstraintId::new();
    (
        CreasePattern {
            vertices: vec![
                Vertex {
                    id: center_vertex,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: source_vertex,
                    position: Point2::new(ROTATION_SCALE, 0.0),
                },
                Vertex {
                    id: target_vertex,
                    position: target,
                },
            ],
            edges: Vec::new(),
        },
        GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: vec![GeometricConstraintRecordV1 {
                id: constraint,
                constraint: GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex,
                    source_vertex,
                    target_vertex,
                    angle_degrees: 90.0,
                },
            }],
        },
        target_vertex,
        constraint,
    )
}

fn editor(pattern: CreasePattern, constraints: GeometricConstraintDocumentV1) -> EditorState {
    EditorState::with_document_parts_and_constraints(
        pattern,
        Paper::default(),
        InstructionTimeline::default(),
        constraints,
    )
}

#[test]
fn move_vertices_uses_frozen_admission_while_public_preview_keeps_runtime_bits() {
    let deterministic_target = Point2::new(0.0, ROTATION_SCALE);
    let runtime_radians = 90.0_f64.to_radians();
    let runtime_target = Point2::new(
        ROTATION_SCALE * runtime_radians.cos(),
        ROTATION_SCALE * runtime_radians.sin(),
    );
    assert_ne!(
        (runtime_target.x.to_bits(), runtime_target.y.to_bits()),
        (
            deterministic_target.x.to_bits(),
            deterministic_target.y.to_bits(),
        ),
        "the runtime-local cardinal evaluation must remain distinct from the frozen exact branch"
    );

    let (deterministic_pattern, deterministic_document, _, _) =
        rotation_fixture(deterministic_target);
    assert_eq!(
        crate::constraint_solver::
            verify_deterministic_geometric_constraint_mutation_admission_with_model_support_for_test_v1(
            &deterministic_pattern,
            &deterministic_document,
            true,
        ),
        Ok(())
    );
    assert_eq!(
        crate::constraint_solver::verify_deterministic_geometric_constraint_mutation_admission_v1(
            &deterministic_pattern,
            &deterministic_document,
        ),
        if ori_numeric::deterministic_transcendental_model_supported_v1() {
            Ok(())
        } else {
            Err(crate::ConstraintSolveErrorV1::NonConvergent)
        }
    );
    assert_eq!(
        crate::verify_geometric_constraint_solution_v1(
            &deterministic_pattern,
            &deterministic_document,
            crate::ConstraintSolveLimitsV1::default().residual_tolerance,
        ),
        Err(crate::ConstraintSolveErrorV1::NonConvergent),
        "the public numerical verifier intentionally retains platform preview semantics"
    );

    let (runtime_pattern, runtime_document, _, _) = rotation_fixture(runtime_target);
    assert_eq!(
        crate::verify_geometric_constraint_solution_v1(
            &runtime_pattern,
            &runtime_document,
            crate::ConstraintSolveLimitsV1::default().residual_tolerance,
        ),
        Ok(0.0)
    );
    assert_eq!(
        crate::constraint_solver::
            verify_deterministic_geometric_constraint_mutation_admission_with_model_support_for_test_v1(
            &runtime_pattern,
            &runtime_document,
            true,
        ),
        Err(crate::ConstraintSolveErrorV1::NonConvergent)
    );

    let (initial_pattern, constraints, target_vertex, deterministic_constraint) =
        rotation_fixture(Point2::new(1.0, ROTATION_SCALE));
    let mut deterministic_editor = editor(initial_pattern, constraints);
    let deterministic_command = Command::MoveVertices {
        updates: vec![VertexPositionUpdate {
            vertex: target_vertex,
            position: deterministic_target,
        }],
    };
    assert_eq!(
        deterministic_editor
            .test_model_supported_geometric_constraint_admission(&deterministic_command, true),
        Ok(()),
        "the editor gate must execute the complete frozen residual path even on unsupported hosts",
    );
    if ori_numeric::deterministic_transcendental_model_supported_v1() {
        deterministic_editor
            .execute(0, deterministic_command)
            .expect("the frozen deterministic candidate is admitted");
        assert_eq!(
            deterministic_editor
                .pattern()
                .vertices
                .iter()
                .find(|vertex| vertex.id == target_vertex)
                .expect("target remains present")
                .position,
            deterministic_target
        );
    } else {
        let initial_pattern = deterministic_editor.pattern().clone();
        assert_eq!(
            deterministic_editor.execute(0, deterministic_command),
            Err(CommandError::GeometricConstraintBlocksGeometryMutation {
                constraint: deterministic_constraint,
            }),
            "the production editor path remains fail-closed on unsupported targets",
        );
        assert_eq!(deterministic_editor.pattern(), &initial_pattern);
        assert_eq!(deterministic_editor.revision(), 0);
    }

    let (initial_pattern, constraints, target_vertex, constraint) =
        rotation_fixture(deterministic_target);
    let mut runtime_editor = editor(initial_pattern.clone(), constraints);
    let runtime_command = Command::MoveVertices {
        updates: vec![VertexPositionUpdate {
            vertex: target_vertex,
            position: runtime_target,
        }],
    };
    assert_eq!(
        runtime_editor.test_model_supported_geometric_constraint_admission(&runtime_command, true),
        Err(CommandError::GeometricConstraintBlocksGeometryMutation { constraint }),
        "the injected supported model must still reject runtime-only residual bits",
    );
    assert_eq!(
        runtime_editor.execute(0, runtime_command),
        Err(CommandError::GeometricConstraintBlocksGeometryMutation { constraint })
    );
    assert_eq!(runtime_editor.pattern(), &initial_pattern);
    assert_eq!(runtime_editor.revision(), 0);
}
