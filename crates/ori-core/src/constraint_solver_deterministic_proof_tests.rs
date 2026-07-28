use std::collections::HashMap;

use ori_domain::{
    ConstraintId, CreasePattern, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2,
    Vertex, VertexId,
};
use ori_numeric::{
    deterministic_atan2_v1, deterministic_hypot_v1, deterministic_polar_endpoint_v2,
    deterministic_sin_cos_degrees_v1,
};

use super::*;

fn rotation_fixture(
    target: Point2,
    angle_degrees: f64,
) -> (CreasePattern, GeometricConstraintDocumentV1) {
    let center_vertex = VertexId::new();
    let source_vertex = VertexId::new();
    let target_vertex = VertexId::new();
    (
        CreasePattern {
            vertices: vec![
                Vertex {
                    id: center_vertex,
                    position: Point2::new(0.0, 0.0),
                },
                Vertex {
                    id: source_vertex,
                    position: Point2::new(1.0, 0.0),
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
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::RotationalSymmetry {
                    center_vertex,
                    source_vertex,
                    target_vertex,
                    angle_degrees,
                },
            }],
        },
    )
}

fn positions(pattern: &CreasePattern) -> HashMap<VertexId, Point2> {
    pattern
        .vertices
        .iter()
        .map(|vertex| (vertex.id, vertex.position))
        .collect()
}

#[test]
fn residual_transcendental_models_are_bound_to_distinct_explicit_kernels() {
    let runtime = ResidualTranscendentalModelV1::RuntimePreview;
    let proof = ResidualTranscendentalModelV1::DeterministicProofV1;

    for (x, y) in [(3.0, 4.0), (f64::from_bits(1), f64::from_bits(2))] {
        assert_eq!(
            runtime.hypot(x, y).map(f64::to_bits),
            Ok(x.hypot(y).to_bits())
        );
        assert_eq!(
            proof.hypot(x, y).unwrap().to_bits(),
            deterministic_hypot_v1(x, y).unwrap().to_bits()
        );
    }
    for (y, x) in [(1.0, 1.0), (f64::from_bits(1), -1.0), (-0.0, -0.0)] {
        assert_eq!(
            runtime.atan2(y, x).map(f64::to_bits),
            Ok(y.atan2(x).to_bits())
        );
        assert_eq!(
            proof.atan2(y, x).unwrap().to_bits(),
            deterministic_atan2_v1(y, x).unwrap().to_bits()
        );
    }

    let degrees = 37.5_f64;
    let runtime_radians = degrees.to_radians();
    assert_eq!(
        runtime
            .sin_cos_degrees(degrees)
            .map(|(sin, cos)| (sin.to_bits(), cos.to_bits())),
        Ok((
            runtime_radians.sin().to_bits(),
            runtime_radians.cos().to_bits(),
        ))
    );
    let (proof_sin, proof_cos) = deterministic_sin_cos_degrees_v1(degrees).unwrap();
    assert_eq!(
        proof
            .sin_cos_degrees(degrees)
            .map(|(sin, cos)| (sin.to_bits(), cos.to_bits())),
        Ok((proof_sin.to_bits(), proof_cos.to_bits()))
    );

    let actual = 1.0_f64.atan2(-0.0);
    assert_eq!(
        runtime.fixed_angle_residual(actual, degrees).to_bits(),
        crate::constraints::fixed_angle_residual_binary64_v1(actual, degrees).to_bits()
    );
    assert_eq!(
        proof.fixed_angle_residual(actual, degrees).to_bits(),
        crate::constraints::deterministic_fixed_angle_residual_binary64_v1(actual, degrees)
            .to_bits()
    );
}

#[test]
fn rotation_preview_keeps_platform_bits_while_proof_uses_frozen_bits() {
    let angle_degrees = 37.5;
    let target = Point2::new(2.0, -3.0);
    let (pattern, document) = rotation_fixture(target, angle_degrees);
    let positions = positions(&pattern);

    let runtime_radians = angle_degrees.to_radians();
    let runtime_expected = [
        target.x - runtime_radians.cos(),
        target.y - runtime_radians.sin(),
    ];
    assert_eq!(
        residuals(&pattern, &document, &positions)
            .unwrap()
            .into_iter()
            .map(f64::to_bits)
            .collect::<Vec<_>>(),
        runtime_expected
            .into_iter()
            .map(f64::to_bits)
            .collect::<Vec<_>>()
    );

    let (proof_sin, proof_cos) = deterministic_sin_cos_degrees_v1(angle_degrees).unwrap();
    let proof_expected = [target.x - proof_cos, target.y - proof_sin];
    assert_eq!(
        deterministic_proof_residuals_v1(&pattern, &document, &positions)
            .unwrap()
            .into_iter()
            .map(f64::to_bits)
            .collect::<Vec<_>>(),
        proof_expected
            .into_iter()
            .map(f64::to_bits)
            .collect::<Vec<_>>()
    );
}

#[test]
fn fixed_angle_preview_and_proof_keep_both_sides_of_their_residual_models() {
    let vertex = VertexId::new();
    let first_end = VertexId::new();
    let second_end = VertexId::new();
    let first_edge = EdgeId::new();
    let second_edge = EdgeId::new();
    let pattern = CreasePattern {
        vertices: vec![
            Vertex {
                id: vertex,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: first_end,
                position: Point2::new(2.0, 1.0),
            },
            Vertex {
                id: second_end,
                position: Point2::new(-1.0, 3.0),
            },
        ],
        edges: vec![
            Edge {
                id: first_edge,
                start: vertex,
                end: first_end,
                kind: EdgeKind::Auxiliary,
            },
            Edge {
                id: second_edge,
                start: vertex,
                end: second_end,
                kind: EdgeKind::Auxiliary,
            },
        ],
    };
    let angle_degrees = 37.5;
    let document = GeometricConstraintDocumentV1 {
        schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::FixedAngle {
                vertex,
                first_edge,
                second_edge,
                angle_degrees,
            },
        }],
    };
    let positions = positions(&pattern);
    let first_vector = (2.0_f64, 1.0_f64);
    let second_vector = (-1.0_f64, 3.0_f64);
    let absolute_cross =
        (first_vector.0 * second_vector.1 - first_vector.1 * second_vector.0).abs();
    let dot = first_vector.0 * second_vector.0 + first_vector.1 * second_vector.1;

    let runtime_actual = absolute_cross.atan2(dot);
    let runtime_difference = runtime_actual - angle_degrees.to_radians();
    let runtime_expected = (runtime_difference + core::f64::consts::PI)
        .rem_euclid(2.0 * core::f64::consts::PI)
        - core::f64::consts::PI;
    assert_eq!(
        residuals(&pattern, &document, &positions)
            .unwrap()
            .into_iter()
            .map(f64::to_bits)
            .collect::<Vec<_>>(),
        vec![runtime_expected.to_bits()]
    );

    let proof_actual = deterministic_atan2_v1(absolute_cross, dot).unwrap();
    let proof_expected = crate::constraints::deterministic_fixed_angle_residual_binary64_v1(
        proof_actual,
        angle_degrees,
    );
    assert_eq!(
        deterministic_proof_residuals_v1(&pattern, &document, &positions)
            .unwrap()
            .into_iter()
            .map(f64::to_bits)
            .collect::<Vec<_>>(),
        vec![proof_expected.to_bits()]
    );
}

#[test]
fn exact_certificate_accepts_only_the_frozen_rotation_assignment() {
    let angle_degrees = 37.5;
    let (proof_sin, proof_cos) = deterministic_sin_cos_degrees_v1(angle_degrees).unwrap();
    let proof_target = Point2::new(proof_cos, proof_sin);
    let (proof_pattern, document) = rotation_fixture(proof_target, angle_degrees);
    assert!(matches!(
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&proof_pattern, &document),
        Ok(Some(_))
    ));

    let runtime_radians = angle_degrees.to_radians();
    let runtime_target = Point2::new(runtime_radians.cos(), runtime_radians.sin());
    if runtime_target.x.to_bits() != proof_target.x.to_bits()
        || runtime_target.y.to_bits() != proof_target.y.to_bits()
    {
        let (runtime_pattern, runtime_document) = rotation_fixture(runtime_target, angle_degrees);
        assert_eq!(
            verify_geometric_constraint_solution_v1(
                &runtime_pattern,
                &runtime_document,
                f64::MIN_POSITIVE,
            ),
            Ok(0.0),
            "the numerical preview verifier must retain platform semantics"
        );
        assert_eq!(
            certify_binary64_exact_geometric_constraint_satisfaction_v1(
                &runtime_pattern,
                &runtime_document,
            )
            .expect("the runtime assignment is structurally valid"),
            None,
            "a platform-only zero must not become deterministic proof authority"
        );
    }
}

#[test]
fn editor_cardinal_endpoint_is_an_exact_rotation_proof_but_neighbors_use_libm_bits() {
    let (target_x, target_y) = deterministic_polar_endpoint_v2(0.0, 0.0, 1.0, 90.0).unwrap();
    assert_eq!(
        (target_x.to_bits(), target_y.to_bits()),
        (0, 1.0_f64.to_bits())
    );
    let target = Point2::new(target_x, target_y);
    let (pattern, document) = rotation_fixture(target, 90.0);
    let certificate =
        certify_binary64_exact_geometric_constraint_satisfaction_v1(&pattern, &document)
            .unwrap()
            .expect("the cardinal editor endpoint must be exact proof authority");
    assert_eq!(
        certificate.model_id(),
        "geometric_constraint_deterministic_binary64_exact_satisfaction_v2"
    );
    assert_eq!(
        certificate.transcendental_model_id(),
        ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
    );
    assert!(!certificate.authorizes_project_mutation());
    assert_eq!(
        certificate.replayable_across_runtimes(),
        ori_numeric::deterministic_transcendental_model_supported_v1()
    );

    let below = f64::from_bits(90.0_f64.to_bits() - 1);
    let above = f64::from_bits(90.0_f64.to_bits() + 1);
    for neighbor in [below, above] {
        let (sin, cos) = deterministic_sin_cos_degrees_v1(neighbor).unwrap();
        assert_ne!(
            (sin.to_bits(), cos.to_bits()),
            (1.0_f64.to_bits(), 0.0_f64.to_bits()),
            "only an exact cardinal degree value may use the exact branch"
        );
        let neighbor_target = Point2::new(cos, sin);
        let (neighbor_pattern, neighbor_document) = rotation_fixture(neighbor_target, neighbor);
        assert!(matches!(
            certify_binary64_exact_geometric_constraint_satisfaction_v1(
                &neighbor_pattern,
                &neighbor_document,
            ),
            Ok(Some(_))
        ));
    }
}

#[test]
fn deterministic_proof_transcendentals_fail_closed_on_nonfinite_intermediates() {
    let proof = ResidualTranscendentalModelV1::DeterministicProofV1;
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            proof.hypot(value, 1.0),
            Err(ConstraintSolveErrorV1::NonConvergent)
        );
        assert_eq!(
            proof.atan2(value, 1.0),
            Err(ConstraintSolveErrorV1::NonConvergent)
        );
        assert_eq!(
            proof.sin_cos_degrees(value),
            Err(ConstraintSolveErrorV1::NonConvergent)
        );
    }
}
