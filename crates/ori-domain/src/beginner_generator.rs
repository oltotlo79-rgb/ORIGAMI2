use serde::{Deserialize, Serialize};

use crate::{
    BeginnerFoldTechniqueV1, BeginnerGenerationConstraintsV1, BeginnerSkeletonSegmentV1,
    BeginnerProtrusionSymmetryV1, BeginnerTargetAssetReferenceV1, BeginnerTargetCategoryV1,
    BeginnerTargetPartKindV1, BeginnerTargetPartRecordV1, CreasePattern, Edge, EdgeId, EdgeKind,
    Point2, ProjectId, Vertex, VertexId,
};

pub const BEGINNER_GENERATOR_SCHEMA_VERSION_V1: u32 = 1;
pub const MAX_BEGINNER_GENERATED_CANDIDATES_V1: usize = 3;
pub const MAX_BEGINNER_GENERATOR_INPUT_VERTICES_V1: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerGeneratedPlanKindV1 {
    SymmetricFourLegBase,
    SymmetricWingBase,
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
    pub target_parts: Vec<BeginnerTargetPartRecordV1>,
    pub skeleton_segments: Vec<BeginnerSkeletonSegmentV1>,
    pub target_asset: Option<BeginnerTargetAssetReferenceV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeginnerGeneratorErrorV1 {
    ResourceLimit,
    UnsupportedPaper,
    UnsupportedTechniques,
    MissingTargetCategory,
    MissingRequiredParts,
    UnsupportedAnimalTemplate,
    UnsupportedInsectTemplate,
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
    let part_count = |kind| {
        constraints
            .target_parts
            .iter()
            .find(|part| part.kind == kind)
            .map_or(0, |part| part.count)
    };
    if part_count(BeginnerTargetPartKindV1::Head) != 1
        || part_count(BeginnerTargetPartKindV1::Torso) != 1
    {
        return Err(BeginnerGeneratorErrorV1::MissingRequiredParts);
    }
    let kind = if allows_valley {
        EdgeKind::Valley
    } else {
        EdgeKind::Mountain
    };
    let template = match target_category {
        BeginnerTargetCategoryV1::Animal => {
            if part_count(BeginnerTargetPartKindV1::Leg) != 4
                || constraints.skeleton_segments.len() < 3
                || !has_bilateral_protrusion_count(constraints, 4)
            {
                return Err(BeginnerGeneratorErrorV1::UnsupportedAnimalTemplate);
            }
            symmetric_template(
                namespace,
                source,
                BeginnerGeneratedPlanKindV1::SymmetricFourLegBase,
                kind,
                min_x,
                max_x,
                min_y,
                max_y,
                &[
                    (0.25, 0.0),
                    (0.75, 0.0),
                    (0.25, 1.0),
                    (0.75, 1.0),
                ],
                "symmetric_four_leg_base",
                constraints,
            )
        }
        BeginnerTargetCategoryV1::Insect => {
            if part_count(BeginnerTargetPartKindV1::Wing) != 2
                || constraints.skeleton_segments.len() < 2
                || !has_bilateral_protrusion_count(constraints, 2)
            {
                return Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate);
            }
            symmetric_template(
                namespace,
                source,
                BeginnerGeneratedPlanKindV1::SymmetricWingBase,
                kind,
                min_x,
                max_x,
                min_y,
                max_y,
                &[(0.0, 0.25), (0.0, 0.75), (1.0, 0.25), (1.0, 0.75)],
                "symmetric_wing_base",
                constraints,
            )
        }
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
    let mut plans = vec![template];
    plans.extend(variants.into_iter().take(MAX_BEGINNER_GENERATED_CANDIDATES_V1 - 1).map(
        |(plan_kind, start, end, instruction)| {
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
                target_parts: constraints.target_parts.clone(),
                skeleton_segments: constraints.skeleton_segments.clone(),
                target_asset: constraints.target_asset,
            }
        },
    ));
    Ok(plans)
}

fn has_bilateral_protrusion_count(
    constraints: &BeginnerGenerationConstraintsV1,
    count: u8,
) -> bool {
    constraints.protrusions.iter().any(|target| {
        target.count == count && target.symmetry == BeginnerProtrusionSymmetryV1::Bilateral
    })
}

#[allow(clippy::too_many_arguments)]
fn symmetric_template(
    namespace: ProjectId,
    source: &CreasePattern,
    plan_kind: BeginnerGeneratedPlanKindV1,
    edge_kind: EdgeKind,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
    endpoints: &[(f64, f64)],
    instruction: &str,
    constraints: &BeginnerGenerationConstraintsV1,
) -> BeginnerGeneratedPlanV1 {
    let prefix = format!("beginner-plan-{plan_kind:?}");
    let center = Point2::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
    let center_id = VertexId::derive_v5(namespace, format!("{prefix}-center").as_bytes());
    let mut vertices = vec![Vertex {
        id: center_id,
        position: center,
    }];
    let mut edges = Vec::with_capacity(endpoints.len());
    for (index, (x_ratio, y_ratio)) in endpoints.iter().copied().enumerate() {
        let position = Point2::new(
            min_x + (max_x - min_x) * x_ratio,
            min_y + (max_y - min_y) * y_ratio,
        );
        let id = source
            .vertices
            .iter()
            .find(|vertex| vertex.position == position)
            .map_or_else(
                || VertexId::derive_v5(namespace, format!("{prefix}-v-{index}").as_bytes()),
                |vertex| vertex.id,
            );
        if !vertices.iter().any(|vertex| vertex.id == id) {
            vertices.push(Vertex { id, position });
        }
        edges.push(Edge {
            id: EdgeId::derive_v5(namespace, format!("{prefix}-e-{index}").as_bytes()),
            start: center_id,
            end: id,
            kind: edge_kind,
        });
    }
    BeginnerGeneratedPlanV1 {
        schema_version: BEGINNER_GENERATOR_SCHEMA_VERSION_V1,
        kind: plan_kind,
        crease_pattern: CreasePattern { vertices, edges },
        instruction_codes: vec![instruction.to_owned()],
        target_parts: constraints.target_parts.clone(),
        skeleton_segments: constraints.skeleton_segments.clone(),
        target_asset: constraints.target_asset,
    }
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
            target_parts: vec![
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Head,
                    count: 1,
                },
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Torso,
                    count: 1,
                },
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Leg,
                    count: 4,
                },
            ],
            skeleton_segments: vec![
                skeleton(1, -10, 0, 0, 10),
                skeleton(2, 10, 0, 0, 10),
                skeleton(3, 0, -10, 0, 10),
            ],
            protrusions: vec![bilateral_protrusion(1, 4)],
            ..BeginnerGenerationConstraintsV1::default()
        };
        let first = generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap();
        let second = generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.len(), 3);
        assert_eq!(
            first[0].kind,
            BeginnerGeneratedPlanKindV1::SymmetricFourLegBase
        );
        assert_eq!(first[0].crease_pattern.edges.len(), 4);
        assert!(first[1..].iter().all(|plan| plan.crease_pattern.edges.len() == 1));
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids[..3], &constraints),
            Err(BeginnerGeneratorErrorV1::UnsupportedPaper)
        );
    }

    #[test]
    fn wing_template_is_explicit_and_unsupported_inputs_fail_closed() {
        let namespace = ProjectId::new();
        let ids = ["a", "b", "c", "d"].map(|name| VertexId::derive_v5(namespace, name.as_bytes()));
        let source = CreasePattern {
            vertices: ids
                .into_iter()
                .zip([
                    Point2::new(0.0, 0.0),
                    Point2::new(20.0, 0.0),
                    Point2::new(20.0, 10.0),
                    Point2::new(0.0, 10.0),
                ])
                .map(|(id, position)| Vertex { id, position })
                .collect(),
            edges: Vec::new(),
        };
        let mut constraints = BeginnerGenerationConstraintsV1 {
            target_category: Some(BeginnerTargetCategoryV1::Insect),
            target_parts: vec![
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Head,
                    count: 1,
                },
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Torso,
                    count: 1,
                },
                BeginnerTargetPartRecordV1 {
                    kind: BeginnerTargetPartKindV1::Wing,
                    count: 2,
                },
            ],
            skeleton_segments: vec![
                skeleton(1, -10, 0, 0, 10),
                skeleton(2, 10, 0, 0, 10),
            ],
            protrusions: vec![bilateral_protrusion(1, 2)],
            ..BeginnerGenerationConstraintsV1::default()
        };
        let plans = generate_beginner_plans_v1(namespace, &source, &ids, &constraints).unwrap();
        assert_eq!(plans[0].kind, BeginnerGeneratedPlanKindV1::SymmetricWingBase);
        assert_eq!(plans[0].crease_pattern.edges.len(), 4);
        constraints.protrusions[0].symmetry = BeginnerProtrusionSymmetryV1::None;
        assert_eq!(
            generate_beginner_plans_v1(namespace, &source, &ids, &constraints),
            Err(BeginnerGeneratorErrorV1::UnsupportedInsectTemplate)
        );
    }

    fn skeleton(
        id: u16,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
    ) -> BeginnerSkeletonSegmentV1 {
        BeginnerSkeletonSegmentV1 {
            id,
            start: crate::BeginnerSkeletonPointV1 {
                x_tenths_mm: start_x,
                y_tenths_mm: start_y,
            },
            end: crate::BeginnerSkeletonPointV1 {
                x_tenths_mm: end_x,
                y_tenths_mm: end_y,
            },
            thickness_tenths_mm: 10,
        }
    }

    fn bilateral_protrusion(id: u16, count: u8) -> crate::BeginnerProtrusionTargetV1 {
        crate::BeginnerProtrusionTargetV1 {
            id,
            count,
            length_tenths_mm: 100,
            thickness_tenths_mm: 10,
            position_tenths_mm: [0, 0, 0],
            direction_milli: [1_000, 0, 0],
            symmetry: BeginnerProtrusionSymmetryV1::Bilateral,
            curvature_degrees: 0,
            joint: crate::BeginnerProtrusionJointV1::Hinge,
            motion_degrees: [0, 45],
            side: crate::BeginnerProtrusionSideV1::Either,
            priority: 80,
        }
    }
}
