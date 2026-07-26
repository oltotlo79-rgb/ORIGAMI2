use std::collections::{HashMap, HashSet};

use ori_domain::{CreasePattern, GeometricConstraintDocumentV1, Point2, VertexId};

use super::{ConstraintSolveErrorV1, hard_len, residuals};
use crate::{ConstraintPreflightV1, GeometricConstraintLimitsV1, prepare_geometric_constraints_v1};

/// Opaque runtime-local proof that one complete finite position overlay makes
/// every production binary64 residual in a validated document exact zero.
///
/// Unlike [`super::Binary64ExactConstraintSatisfactionV1`], this algebraic
/// witness deliberately does not claim that the overlay is a valid crease
/// pattern geometry. Referenced edge endpoints may coincide. The source
/// pattern itself remains fully validated and immutable, and all edge and
/// constraint topology is read exclusively from that source.
///
/// This type intentionally implements neither `Clone` nor serialization and
/// exposes no project-mutation or cross-runtime replay authority.
pub(crate) struct Binary64ResidualOnlyConstraintSatisfactionV1 {
    _constraint_count: usize,
    _equation_count: usize,
    _vertex_count: usize,
}

/// Certifies a complete algebraic position overlay without weakening the
/// ordinary non-degenerate pattern certificate.
///
/// The source pattern and document first pass the complete ordinary V1
/// preparation boundary. The overlay must then be an exact bijection over all
/// source vertex IDs: no missing, duplicate, unknown, or non-finite entry is
/// admitted. Residual evaluation uses the source topology and the production
/// evaluator unchanged. Coincident overlay coordinates are permitted only in
/// this private residual-only path.
pub(crate) fn certify_binary64_residual_only_constraint_overlay_v1(
    source_pattern: &CreasePattern,
    document: &GeometricConstraintDocumentV1,
    overlay: &[(VertexId, Point2)],
) -> Result<Option<Binary64ResidualOnlyConstraintSatisfactionV1>, ConstraintSolveErrorV1> {
    let prepared = prepare_geometric_constraints_v1(
        source_pattern,
        document,
        GeometricConstraintLimitsV1::default(),
    )
    .map_err(|_| ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry)?;
    if matches!(
        prepared.preflight(),
        ConstraintPreflightV1::DirectConflict { .. }
    ) {
        return Err(ConstraintSolveErrorV1::NonConvergent);
    }

    let positions = validate_complete_overlay(source_pattern, overlay)?;
    let equation_count = hard_len(document)?;
    let values = residuals(source_pattern, document, &positions)?;
    debug_assert_eq!(values.len(), equation_count);
    if !values.iter().all(|value| *value == 0.0) {
        return Ok(None);
    }
    Ok(Some(Binary64ResidualOnlyConstraintSatisfactionV1 {
        _constraint_count: document.constraints.len(),
        _equation_count: equation_count,
        _vertex_count: source_pattern.vertices.len(),
    }))
}

fn validate_complete_overlay(
    source_pattern: &CreasePattern,
    overlay: &[(VertexId, Point2)],
) -> Result<HashMap<VertexId, Point2>, ConstraintSolveErrorV1> {
    if overlay.len() != source_pattern.vertices.len() {
        return Err(ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry);
    }
    let source_ids = source_pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<HashSet<_>>();
    let mut positions = HashMap::with_capacity(overlay.len());
    for (vertex, point) in overlay {
        if !source_ids.contains(vertex)
            || !point.x.is_finite()
            || !point.y.is_finite()
            || positions.insert(*vertex, *point).is_some()
        {
            return Err(ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry);
        }
    }
    if positions.len() != source_ids.len() {
        return Err(ConstraintSolveErrorV1::InvalidConstraintDocumentOrGeometry);
    }
    Ok(positions)
}
