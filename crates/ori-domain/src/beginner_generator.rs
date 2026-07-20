use serde::{Deserialize, Serialize};

use crate::{
    BeginnerFoldTechniqueV1, BeginnerGenerationConstraintsV1, BeginnerTargetCategoryV1,
    CreasePattern, Edge, EdgeId, EdgeKind, Point2, ProjectId, Vertex, VertexId,
};

pub const BEGINNER_GENERATOR_SCHEMA_VERSION_V1: u32 = 1;
pub const MAX_BEGINNER_GENERATED_CANDIDATES_V1: usize = 3;
pub const MAX_BEGINNER_GENERATOR_INPUT_VERTICES_V1: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerGeneratedPlanKindV1 {
    VerticalBookFold,
    HorizontalBookFold,
    DiagonalFold,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerGeneratedPlanV1 {
    pub schema_version: u32,
    pub kind: BeginnerGeneratedPlanKindV1,
    pub crease_pattern: CreasePattern,
    pub instruction_codes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeginnerGeneratorErrorV1 {
    ResourceLimit,
    UnsupportedPaper,
    UnsupportedTechniques,
    MissingTargetCategory,
}

pub fn generate_beginner_plans_v1(
    namespace: ProjectId,
    source: &CreasePattern,
    boundary_vertices: &[VertexId],
    constraints: &BeginnerGenerationConstraintsV1,
) -> Result<Vec<BeginnerGeneratedPlanV1>, BeginnerGeneratorErrorV1> {
    if source.vertices.len() > MAX_BEGINNER_GENERATOR_INPUT_VERTICES_V1 {
        return Err(BeginnerGeneratorErrorV1::ResourceLimit);
    }
    if boundary_vertices.len() != 4 {
        return Err(BeginnerGeneratorErrorV1::UnsupportedPaper);
    }
    let points = boundary_vertices
        .iter()
        .map(|id| {
            source
                .vertices
                .iter()
                .find(|vertex| vertex.id == *id)
                .map(|vertex| vertex.position)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(BeginnerGeneratorErrorV1::UnsupportedPaper)?;
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    if ![min_x, max_x, min_y, max_y].into_iter().all(f64::is_finite)
        || min_x >= max_x
        || min_y >= max_y
        || !points.iter().all(|point| {
            (point.x == min_x || point.x == max_x) && (point.y == min_y || point.y == max_y)
        })
    {
        return Err(BeginnerGeneratorErrorV1::UnsupportedPaper);
    }
    let allows_valley = constraints
        .allowed_techniques
        .contains(&BeginnerFoldTechniqueV1::ValleyFold);
    let allows_mountain = constraints
        .allowed_techniques
        .contains(&BeginnerFoldTechniqueV1::MountainFold);
    if !allows_valley && !allows_mountain {
        return Err(BeginnerGeneratorErrorV1::UnsupportedTechniques);
    }
    let target_category = constraints
        .target_category
        .ok_or(BeginnerGeneratorErrorV1::MissingTargetCategory)?;
    let kind = if allows_valley {
        EdgeKind::Valley
    } else {
        EdgeKind::Mountain
    };
    let animal_variants = [
        (
            BeginnerGeneratedPlanKindV1::VerticalBookFold,
            Point2::new((min_x + max_x) / 2.0, min_y),
            Point2::new((min_x + max_x) / 2.0, max_y),
            "book_fold_vertical",
        ),
        (
            BeginnerGeneratedPlanKindV1::HorizontalBookFold,
            Point2::new(min_x, (min_y + max_y) / 2.0),
            Point2::new(max_x, (min_y + max_y) / 2.0),
            "book_fold_horizontal",
        ),
        (
            BeginnerGeneratedPlanKindV1::DiagonalFold,
            Point2::new(min_x, min_y),
            Point2::new(max_x, max_y),
            "diagonal_fold",
        ),
    ];
    let variants = match target_category {
        BeginnerTargetCategoryV1::Animal => animal_variants,
        BeginnerTargetCategoryV1::Insect => {
            [animal_variants[2], animal_variants[0], animal_variants[1]]
        }
    };
    Ok(variants
        .into_iter()
        .take(MAX_BEGINNER_GENERATED_CANDIDATES_V1)
        .map(|(plan_kind, start, end, instruction)| {
            let prefix = format!("beginner-plan-{plan_kind:?}");
            let start_id = source
                .vertices
                .iter()
                .find(|vertex| vertex.position == start)
                .map_or_else(
                    || VertexId::derive_v5(namespace, format!("{prefix}-start").as_bytes()),
                    |vertex| vertex.id,
                );
            let end_id = source
                .vertices
                .iter()
                .find(|vertex| vertex.position == end)
                .map_or_else(
                    || VertexId::derive_v5(namespace, format!("{prefix}-end").as_bytes()),
                    |vertex| vertex.id,
                );
            BeginnerGeneratedPlanV1 {
                schema_version: BEGINNER_GENERATOR_SCHEMA_VERSION_V1,
                kind: plan_kind,
                crease_pattern: CreasePattern {
                    vertices: vec![
                        Vertex {
                            id: start_id,
                            position: start,
                        },
                        Vertex {
                            id: end_id,
                            position: end,
                        },
                    ],
                    edges: vec![Edge {
                        id: EdgeId::derive_v5(namespace, format!("{prefix}-edge").as_bytes()),
                        start: start_id,
                        end: end_id,
                        kind,
                    }],
                },
                instruction_codes: vec![instruction.to_owned()],
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_is_bounded_deterministic_and_fail_closed() {
        let namespace = ProjectId::new();
        let ids = ["a", "b", "c", "d"].map(|name| VertexId::derive_v5(namespace, name.as_bytes()));
        let source = CreasePattern {
            vertices: ids
                .into_iter()
                .zip([
                    Point2::new(0.0, 0.0),
                    Point2::new(10.0, 0.0),
                    Point2::new(10.0, 10.0),
                    Point2::new(0.0, 10.0),
                ])
                .map(|(id, position)| Vertex { id, position })
                .collect(),
            edges: Vec::new(),
        };
        let constraints = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::Animal),
            ..BeginnerGenerationConstraintsV1::default()
        };
        let first = generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap();
        let second = generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.len(), 3);
        assert!(
            first
                .iter()
                .all(|plan| plan.crease_pattern.edges.len() == 1)
        );
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids[..3], &constraints),
            Err(BeginnerGeneratorErrorV1::UnsupportedPaper)
        );
    }
}
