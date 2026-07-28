//! Deterministic, fail-closed admission for constrained geometry mutation.
//!
//! Numerical solve previews and the public tolerance verifier intentionally
//! retain runtime-local floating-point transcendental semantics. Project
//! mutation cannot use that platform-dependent boundary. This module instead
//! re-evaluates the complete candidate and complete constraint document with
//! the frozen proof residual algebra, but returns no witness, certificate, or
//! reusable capability.

use std::collections::HashMap;

use ori_domain::{CreasePattern, GeometricConstraintDocumentV1};
use ori_numeric::deterministic_transcendental_model_supported_v1;

use crate::{
    ConstraintPreflightV1, GeometricConstraintLimitsV1, GeometricConstraintUnknownReasonV1,
    prepare_geometric_constraints_v1,
};

use super::{ConstraintSolveErrorV1, deterministic_proof_residuals_v1, hard_len, maximum_absolute};

// Freeze the binary64 tolerance as part of the V1 mutation-admission model.
// This is the exact bit pattern previously obtained from the solver default;
// changing a numerical-preview default must not silently move this boundary.
const MUTATION_ADMISSION_RESIDUAL_TOLERANCE_V1: f64 = f64::from_bits(0x3e7a_d7f2_9abc_af48);

fn preflight_permits_complete_deterministic_residual_evaluation_v1(
    preflight: &ConstraintPreflightV1,
) -> bool {
    matches!(
        preflight,
        ConstraintPreflightV1::NoDirectConflict
            | ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                ..
            }
    )
}

/// Revalidates one complete candidate for the constrained mutation boundary.
///
/// The returned unit value is deliberately ephemeral. It cannot be serialized,
/// retained, or promoted into proof authority. Unsupported targets, positive
/// direct conflicts, resource/cancellation preflight failures, invalid
/// resources, allocation failure, non-finite intermediates, and any residual
/// above the fixed tolerance all fail closed. The narrow
/// `SolverRequiredConstraintKinds` preflight outcome proceeds only to the
/// complete deterministic residual evaluation below; it is not accepted by
/// itself.
pub(crate) fn verify_deterministic_geometric_constraint_mutation_admission_v1(
    candidate: &CreasePattern,
    constraints: &GeometricConstraintDocumentV1,
) -> Result<(), ConstraintSolveErrorV1> {
    verify_deterministic_geometric_constraint_mutation_admission_with_model_support_v1(
        candidate,
        constraints,
        deterministic_transcendental_model_supported_v1(),
    )
}

fn verify_deterministic_geometric_constraint_mutation_admission_with_model_support_v1(
    candidate: &CreasePattern,
    constraints: &GeometricConstraintDocumentV1,
    deterministic_model_supported: bool,
) -> Result<(), ConstraintSolveErrorV1> {
    if !deterministic_model_supported {
        return Err(ConstraintSolveErrorV1::NonConvergent);
    }

    let prepared = prepare_geometric_constraints_v1(
        candidate,
        constraints,
        GeometricConstraintLimitsV1::default(),
    )
    .map_err(|_| ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry)?;
    if !preflight_permits_complete_deterministic_residual_evaluation_v1(&prepared.preflight()) {
        return Err(ConstraintSolveErrorV1::NonConvergent);
    }

    // `hard_len` is an exhaustive inventory of the persisted constraint enum.
    // A newly added family cannot become mutation-admissible until both that
    // inventory and the shared residual evaluator implement it.
    let equation_count = hard_len(constraints)?;
    let mut positions = HashMap::new();
    positions
        .try_reserve(candidate.vertices.len())
        .map_err(|_| ConstraintSolveErrorV1::WorkLimitExceeded)?;
    for vertex in &candidate.vertices {
        if positions.insert(vertex.id, vertex.position).is_some() {
            return Err(ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry);
        }
    }

    let residuals = deterministic_proof_residuals_v1(candidate, constraints, &positions)?;
    if residuals.len() != equation_count {
        return Err(ConstraintSolveErrorV1::UnsupportedConstraintKind);
    }
    if maximum_absolute(&residuals) <= MUTATION_ADMISSION_RESIDUAL_TOLERANCE_V1 {
        Ok(())
    } else {
        Err(ConstraintSolveErrorV1::NonConvergent)
    }
}

#[cfg(test)]
pub(crate) fn verify_deterministic_geometric_constraint_mutation_admission_with_model_support_for_test_v1(
    candidate: &CreasePattern,
    constraints: &GeometricConstraintDocumentV1,
    deterministic_model_supported: bool,
) -> Result<(), ConstraintSolveErrorV1> {
    verify_deterministic_geometric_constraint_mutation_admission_with_model_support_v1(
        candidate,
        constraints,
        deterministic_model_supported,
    )
}

#[cfg(test)]
mod tests {
    use ori_domain::{
        ConstraintId, Edge, EdgeId, EdgeKind, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        GeometricConstraintKindV1, GeometricConstraintRecordV1, Point2, Vertex, VertexId,
    };
    use ori_numeric::deterministic_sin_cos_degrees_v1;

    use super::*;

    #[derive(Default)]
    struct Fixture {
        vertices: Vec<Vertex>,
        edges: Vec<Edge>,
        constraints: Vec<GeometricConstraintRecordV1>,
    }

    impl Fixture {
        fn vertex(&mut self, x: f64, y: f64) -> VertexId {
            let id = VertexId::new();
            self.vertices.push(Vertex {
                id,
                position: Point2::new(x, y),
            });
            id
        }

        fn edge(&mut self, start: VertexId, end: VertexId) -> EdgeId {
            let id = EdgeId::new();
            self.edges.push(Edge {
                id,
                start,
                end,
                kind: EdgeKind::Auxiliary,
            });
            id
        }

        fn constraint(&mut self, constraint: GeometricConstraintKindV1) {
            self.constraints.push(GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint,
            });
        }

        fn finish(self) -> (CreasePattern, GeometricConstraintDocumentV1) {
            (
                CreasePattern {
                    vertices: self.vertices,
                    edges: self.edges,
                },
                GeometricConstraintDocumentV1 {
                    schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
                    constraints: self.constraints,
                },
            )
        }
    }

    fn all_family_fixture() -> (CreasePattern, GeometricConstraintDocumentV1) {
        let mut fixture = Fixture::default();
        let origin = fixture.vertex(0.0, 0.0);
        let x_one = fixture.vertex(1.0, 0.0);
        let x_two = fixture.vertex(2.0, 0.0);
        let y_one = fixture.vertex(0.0, 1.0);
        let y_two = fixture.vertex(0.0, 2.0);
        let diagonal = fixture.vertex(1.0, 1.0);
        let below = fixture.vertex(0.0, -1.0);
        let (rotation_sin, rotation_cos) =
            deterministic_sin_cos_degrees_v1(90.0).expect("frozen cardinal rotation");
        let rotated_x_one = fixture.vertex(rotation_cos, rotation_sin);
        let parallel_start = fixture.vertex(0.0, 3.0);
        let parallel_end = fixture.vertex(2.0, 3.0);

        let horizontal_one = fixture.edge(origin, x_one);
        let horizontal_two = fixture.edge(origin, x_two);
        let vertical_one = fixture.edge(origin, y_one);
        let _vertical_two = fixture.edge(origin, y_two);
        let diagonal_edge = fixture.edge(origin, diagonal);
        let parallel_offset = fixture.edge(parallel_start, parallel_end);

        fixture.constraint(GeometricConstraintKindV1::FixedLength {
            edge: horizontal_two,
            length_mm: 2.0,
        });
        fixture.constraint(GeometricConstraintKindV1::FixedAngle {
            vertex: origin,
            first_edge: horizontal_one,
            second_edge: horizontal_two,
            angle_degrees: 0.0,
        });
        fixture.constraint(GeometricConstraintKindV1::Horizontal {
            edge: horizontal_one,
        });
        fixture.constraint(GeometricConstraintKindV1::Vertical { edge: vertical_one });
        fixture.constraint(GeometricConstraintKindV1::EqualLength {
            first_edge: horizontal_one,
            second_edge: vertical_one,
        });
        fixture.constraint(GeometricConstraintKindV1::Parallel {
            first_edge: horizontal_two,
            second_edge: parallel_offset,
        });
        fixture.constraint(GeometricConstraintKindV1::PointOnLine {
            vertex: x_one,
            line_edge: horizontal_two,
        });
        fixture.constraint(GeometricConstraintKindV1::MirrorSymmetry {
            first_vertex: y_one,
            second_vertex: below,
            axis_edge: horizontal_two,
        });
        fixture.constraint(GeometricConstraintKindV1::RotationalSymmetry {
            center_vertex: origin,
            source_vertex: x_one,
            target_vertex: rotated_x_one,
            angle_degrees: 90.0,
        });
        fixture.constraint(GeometricConstraintKindV1::AngleBisector {
            vertex: origin,
            first_edge: horizontal_one,
            second_edge: vertical_one,
            bisector_edge: diagonal_edge,
        });
        fixture.constraint(GeometricConstraintKindV1::LengthRatio {
            numerator_edge: horizontal_two,
            denominator_edge: horizontal_one,
            ratio: 2.0,
        });

        fixture.finish()
    }

    fn horizontal_fixture(end_y: f64) -> (CreasePattern, GeometricConstraintDocumentV1) {
        let mut fixture = Fixture::default();
        let start = fixture.vertex(0.0, 0.0);
        let end = fixture.vertex(1.0, end_y);
        let edge = fixture.edge(start, end);
        fixture.constraint(GeometricConstraintKindV1::Horizontal { edge });
        fixture.finish()
    }

    #[test]
    fn all_eleven_persisted_families_reach_the_same_deterministic_admission() {
        let (pattern, document) = all_family_fixture();
        assert_eq!(document.constraints.len(), 11);
        assert_eq!(hard_len(&document), Ok(14));
        assert!(matches!(
            prepare_geometric_constraints_v1(
                &pattern,
                &document,
                GeometricConstraintLimitsV1::default(),
            )
            .expect("all-family fixture prepares")
            .preflight(),
            ConstraintPreflightV1::Unknown {
                reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                ..
            }
        ));
        assert_eq!(
            verify_deterministic_geometric_constraint_mutation_admission_with_model_support_for_test_v1(
                &pattern,
                &document,
                true,
            ),
            Ok(())
        );
        assert_eq!(
            verify_deterministic_geometric_constraint_mutation_admission_v1(&pattern, &document),
            if deterministic_transcendental_model_supported_v1() {
                Ok(())
            } else {
                Err(ConstraintSolveErrorV1::NonConvergent)
            }
        );
    }

    #[test]
    fn fixed_tolerance_accepts_its_exact_bit_and_rejects_one_ulp_above() {
        assert_eq!(
            MUTATION_ADMISSION_RESIDUAL_TOLERANCE_V1.to_bits(),
            0x3e7a_d7f2_9abc_af48
        );
        let (at_pattern, at_document) =
            horizontal_fixture(MUTATION_ADMISSION_RESIDUAL_TOLERANCE_V1);
        assert_eq!(
            verify_deterministic_geometric_constraint_mutation_admission_with_model_support_for_test_v1(
                &at_pattern,
                &at_document,
                true,
            ),
            Ok(())
        );

        let above = f64::from_bits(MUTATION_ADMISSION_RESIDUAL_TOLERANCE_V1.to_bits() + 1);
        let (above_pattern, above_document) = horizontal_fixture(above);
        assert_eq!(
            verify_deterministic_geometric_constraint_mutation_admission_with_model_support_for_test_v1(
                &above_pattern,
                &above_document,
                true,
            ),
            Err(ConstraintSolveErrorV1::NonConvergent)
        );
    }

    #[test]
    fn nonfinite_overflow_and_direct_conflict_fail_closed() {
        let (mut nonfinite, document) = horizontal_fixture(0.0);
        nonfinite.vertices[1].position.y = f64::NAN;
        assert!(
            verify_deterministic_geometric_constraint_mutation_admission_with_model_support_for_test_v1(
                &nonfinite,
                &document,
                true,
            )
            .is_err()
        );

        let (mut overflow, document) = horizontal_fixture(0.0);
        overflow.vertices[0].position.y = -f64::MAX;
        overflow.vertices[1].position.y = f64::MAX;
        assert!(
            verify_deterministic_geometric_constraint_mutation_admission_with_model_support_for_test_v1(
                &overflow,
                &document,
                true,
            )
            .is_err()
        );

        let (pattern, mut expanded_document) = horizontal_fixture(0.0);
        let edge = pattern.edges[0].id;
        expanded_document
            .constraints
            .push(GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Vertical { edge },
            });
        assert_eq!(
            verify_deterministic_geometric_constraint_mutation_admission_with_model_support_for_test_v1(
                &pattern,
                &expanded_document,
                true,
            ),
            Err(ConstraintSolveErrorV1::NonConvergent)
        );
    }

    #[test]
    fn outer_exact_record_limit_still_rejects_preflight_constraint_limit_and_one_over() {
        let mut fixture = Fixture::default();
        let origin = fixture.vertex(0.0, 0.0);
        let x_one = fixture.vertex(1.0, 0.0);
        let y_one = fixture.vertex(0.0, 1.0);
        let diagonal = fixture.vertex(1.0, 1.0);
        let first_edge = fixture.edge(origin, x_one);
        let second_edge = fixture.edge(origin, y_one);
        let bisector_edge = fixture.edge(origin, diagonal);
        let (pattern, _) = fixture.finish();
        let document = |count| GeometricConstraintDocumentV1 {
            schema_version: GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints: (0..count)
                .map(|_| GeometricConstraintRecordV1 {
                    id: ConstraintId::new(),
                    constraint: GeometricConstraintKindV1::AngleBisector {
                        vertex: origin,
                        first_edge,
                        second_edge,
                        bisector_edge,
                    },
                })
                .collect(),
        };
        let exact = document(ori_domain::DEFAULT_MAX_CONSTRAINT_RECORDS);
        // The outer document validator admits exact equality at its hard
        // record/reference ceilings. That is not mutation authority: this
        // fixture exceeds the narrower direct-analysis constraint limit and
        // must therefore remain fail-closed before residual evaluation.
        let exact_preflight = prepare_geometric_constraints_v1(
            &pattern,
            &exact,
            GeometricConstraintLimitsV1::default(),
        )
        .expect("the exact hard ceiling prepares")
        .preflight();
        assert!(
            matches!(
                exact_preflight,
                ConstraintPreflightV1::Unknown {
                    reason: GeometricConstraintUnknownReasonV1::ConstraintLimitExceeded,
                    ..
                }
            ),
            "the hard document ceiling prepares, but the narrower direct-analysis ceiling must remain explicit",
        );
        assert_eq!(
            verify_deterministic_geometric_constraint_mutation_admission_with_model_support_for_test_v1(
                &pattern,
                &exact,
                true,
            ),
            Err(ConstraintSolveErrorV1::NonConvergent)
        );
        let one_over = document(ori_domain::DEFAULT_MAX_CONSTRAINT_RECORDS + 1);
        assert_eq!(
            verify_deterministic_geometric_constraint_mutation_admission_with_model_support_for_test_v1(
                &pattern,
                &one_over,
                true,
            ),
            Err(ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry)
        );
    }

    #[test]
    fn unsupported_model_support_fails_closed_before_admission() {
        let (pattern, document) = horizontal_fixture(0.0);
        assert_eq!(
            verify_deterministic_geometric_constraint_mutation_admission_with_model_support_for_test_v1(
                &pattern,
                &document,
                false,
            ),
            Err(ConstraintSolveErrorV1::NonConvergent)
        );
    }

    #[test]
    fn only_solver_required_unknown_may_reach_complete_residual_evaluation() {
        assert!(
            preflight_permits_complete_deterministic_residual_evaluation_v1(
                &ConstraintPreflightV1::NoDirectConflict,
            )
        );
        assert!(
            preflight_permits_complete_deterministic_residual_evaluation_v1(
                &ConstraintPreflightV1::Unknown {
                    reason: GeometricConstraintUnknownReasonV1::SolverRequiredConstraintKinds,
                    unchecked_constraint_ids: Vec::new(),
                },
            )
        );
        assert!(
            !preflight_permits_complete_deterministic_residual_evaluation_v1(
                &ConstraintPreflightV1::DirectConflict {
                    conflicts: Vec::new(),
                },
            )
        );
        for reason in [
            GeometricConstraintUnknownReasonV1::WorkLimitExceeded,
            GeometricConstraintUnknownReasonV1::ConstraintLimitExceeded,
            GeometricConstraintUnknownReasonV1::StorageLimitExceeded,
            GeometricConstraintUnknownReasonV1::Cancelled,
            GeometricConstraintUnknownReasonV1::DeadlineReached,
        ] {
            assert!(
                !preflight_permits_complete_deterministic_residual_evaluation_v1(
                    &ConstraintPreflightV1::Unknown {
                        reason,
                        unchecked_constraint_ids: Vec::new(),
                    },
                ),
                "{reason:?} must fail closed before residual evaluation",
            );
        }
    }
}
