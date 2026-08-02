use super::*;
use crate::NoopGlobalFlatFoldabilityObserver;
use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId};
use ori_topology::{FaceExtractionInput, extract_faces_strict};
use serde::de::DeserializeOwned;

fn fixed_id<T: DeserializeOwned>(suffix: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\""))
        .expect("fixed UUID fixture")
}

fn zero_work() -> GlobalFlatFoldabilityWorkCounts {
    GlobalFlatFoldabilityWorkCounts {
        source_vertex_records: 0,
        source_edge_records: 0,
        paper_boundary_vertex_records: 0,
        face_records: 0,
        face_boundary_half_edges: 0,
        hinge_records: 0,
        edge_incidence_records: 0,
        local_vertex_records: 0,
        total_records: 0,
        overlap_face_pairs: 0,
        arrangement_segments: 0,
        overlap_cells: 0,
        constraints: 0,
        search_nodes: 0,
        exact_operations: 0,
        exact_values: 0,
        certificate_bytes: 0,
    }
}

struct DeadlineAfter {
    continued_checkpoints: usize,
}

impl GlobalFlatFoldabilityObserver for DeadlineAfter {
    fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
        if self.continued_checkpoints == 0 {
            GlobalFlatFoldabilityCheckpoint::DeadlineReached
        } else {
            self.continued_checkpoints -= 1;
            GlobalFlatFoldabilityCheckpoint::Continue
        }
    }
}

struct AlwaysCancel;

impl GlobalFlatFoldabilityObserver for AlwaysCancel {
    fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
        GlobalFlatFoldabilityCheckpoint::Cancelled
    }
}

#[derive(Default)]
struct CountingObserver {
    checkpoints: usize,
}

impl GlobalFlatFoldabilityObserver for CountingObserver {
    fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
        self.checkpoints += 1;
        GlobalFlatFoldabilityCheckpoint::Continue
    }
}

struct CancelAfter {
    continued_checkpoints: usize,
}

impl GlobalFlatFoldabilityObserver for CancelAfter {
    fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
        if self.continued_checkpoints == 0 {
            GlobalFlatFoldabilityCheckpoint::Cancelled
        } else {
            self.continued_checkpoints -= 1;
            GlobalFlatFoldabilityCheckpoint::Continue
        }
    }
}

fn integer_point(x: i64, y: i64) -> Point {
    Point {
        x: Rational::from_integer(x.into()),
        y: Rational::from_integer(y.into()),
    }
}

fn three_panel_accordion() -> (Paper, CreasePattern, TopologySnapshot) {
    let vertices = (0..8)
        .map(|index| fixed_id::<VertexId>(0x100 + index))
        .collect::<Vec<_>>();
    let positions = [
        Point2::new(0.0, 0.0),
        Point2::new(2.0, 0.0),
        Point2::new(4.0, 0.0),
        Point2::new(6.0, 0.0),
        Point2::new(6.0, 2.0),
        Point2::new(4.0, 2.0),
        Point2::new(2.0, 2.0),
        Point2::new(0.0, 2.0),
    ];
    let vertex_records = vertices
        .iter()
        .copied()
        .zip(positions)
        .map(|(id, position)| Vertex { id, position })
        .collect::<Vec<_>>();
    let mut edges = (0..vertices.len())
        .map(|index| Edge {
            id: fixed_id(0x200 + index as u64),
            start: vertices[index],
            end: vertices[(index + 1) % vertices.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.push(Edge {
        id: fixed_id(0x301),
        start: vertices[1],
        end: vertices[6],
        kind: EdgeKind::Mountain,
    });
    edges.push(Edge {
        id: fixed_id(0x302),
        start: vertices[2],
        end: vertices[5],
        kind: EdgeKind::Valley,
    });
    let paper = Paper {
        boundary_vertices: vertices,
        ..Paper::default()
    };
    let pattern = CreasePattern {
        vertices: vertex_records,
        edges,
    };
    let topology = extract_faces_strict(FaceExtractionInput {
        identity_namespace: fixed_id::<ProjectId>(1),
        source_revision: 73,
        paper: &paper,
        pattern: &pattern,
    })
    .expect("three-panel accordion topology");
    (paper, pattern, topology)
}

fn synthetic_face(index: usize, polygon: Vec<Point>, front_up: bool) -> FoldedFace {
    let layer = LayerFace {
        face_id: fixed_id(0x900 + index as u64),
        face_key: ori_topology::FaceKey([u8::try_from(index + 1).unwrap_or(255); 32]),
    };
    FoldedFace {
        source: SourceFace {
            layer,
            vertex_ids: Vec::new(),
            source_polygon: polygon.clone(),
        },
        transform: Transform::identity(),
        front_up,
        polygon,
    }
}

fn synthetic_cell(key: u8, boundary: Vec<Point>, covering_faces: Vec<usize>) -> OverlapCell {
    OverlapCell {
        key: OverlapCellKey([key; 32]),
        boundary,
        covering_faces,
    }
}

fn all_pairs(face_count: usize) -> Vec<OverlapPair> {
    (0..face_count)
        .flat_map(|first| {
            ((first + 1)..face_count).map(move |second| OverlapPair { first, second })
        })
        .collect()
}

fn rectangle(min_x: i64, min_y: i64, max_x: i64, max_y: i64) -> Vec<Point> {
    vec![
        integer_point(min_x, min_y),
        integer_point(max_x, min_y),
        integer_point(max_x, max_y),
        integer_point(min_x, max_y),
    ]
}

fn build_test_arrangement(faces: &[FoldedFace]) -> (Vec<OverlapPair>, Vec<OverlapCell>) {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let pairs = build_overlap_pairs(faces, &mut runtime).expect("test overlap pairs");
    let cells = build_overlap_cells(faces, &pairs, &mut runtime).expect("test overlap cells");
    (pairs, cells)
}

fn geometry_arrangement_signature(
    paper: &Paper,
    pattern: &CreasePattern,
    topology: &TopologySnapshot,
) -> Vec<(OverlapCellKey, Vec<Point>, Vec<LayerFace>)> {
    let mut canonical_faces = topology
        .faces
        .iter()
        .map(|face| LayerFace {
            face_id: face.id,
            face_key: face.key,
        })
        .collect::<Vec<_>>();
    canonical_faces.sort_unstable_by_key(|face| (face.face_key, face.face_id.canonical_bytes()));
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    runtime
        .advance(
            GlobalFlatFoldabilityPhase::BuildingFlatEmbedding,
            Some(canonical_faces.len()),
        )
        .expect("embedding phase");
    let embedding = build_flat_embedding(paper, pattern, topology, &canonical_faces, &mut runtime)
        .expect("deterministic embedding");
    runtime
        .advance(GlobalFlatFoldabilityPhase::BuildingOverlapArrangement, None)
        .expect("arrangement phase");
    let pairs = build_overlap_pairs(&embedding.faces, &mut runtime).expect("overlap pairs");
    build_overlap_cells(&embedding.faces, &pairs, &mut runtime)
        .expect("canonical cells")
        .into_iter()
        .map(|cell| {
            (
                cell.key,
                cell.boundary,
                cell.covering_faces
                    .into_iter()
                    .map(|index| embedding.faces[index].source.layer)
                    .collect(),
            )
        })
        .collect()
}

fn arrangement_boundary_bytes(cells: &[OverlapCell]) -> usize {
    cells
        .iter()
        .map(|cell| exact_storage_bytes_points(&cell.boundary).expect("cell exact bytes"))
        .fold(0_usize, usize::saturating_add)
}

fn overlap_cell_signatures(cells: &[OverlapCell]) -> Vec<(OverlapCellKey, Vec<Point>, Vec<usize>)> {
    cells
        .iter()
        .map(|cell| (cell.key, cell.boundary.clone(), cell.covering_faces.clone()))
        .collect()
}

fn build_cells_with_supporting_line_deduplication_mode(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    limits: GlobalFlatFoldabilityLimits,
    deduplicate_supporting_lines: bool,
) -> FacewiseResult<(
    Vec<OverlapCell>,
    GlobalFlatFoldabilityWorkCounts,
    ExactStorage,
)> {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(&mut observer, limits, zero_work());
    let cells = if deduplicate_supporting_lines {
        build_overlap_cells(faces, pairs, &mut runtime)
    } else {
        build_overlap_cells_without_supporting_line_deduplication(faces, pairs, &mut runtime)
    }?;
    Ok((cells, runtime.work, runtime.exact_storage))
}

fn build_cells_with_region_face_bounds_pruning_mode(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    prune_strictly_separated_bounds: bool,
) -> FacewiseResult<(
    Vec<OverlapCell>,
    GlobalFlatFoldabilityWorkCounts,
    ExactStorage,
)> {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let cells = if prune_strictly_separated_bounds {
        build_overlap_cells(faces, pairs, &mut runtime)
    } else {
        build_overlap_cells_without_region_face_bounds_pruning(faces, pairs, &mut runtime)
    }?;
    Ok((cells, runtime.work, runtime.exact_storage))
}

fn build_cells_with_prevalidated_region_reuse_mode(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    limits: GlobalFlatFoldabilityLimits,
    reuse_prevalidated_regions: bool,
) -> FacewiseResult<(
    Vec<OverlapCell>,
    GlobalFlatFoldabilityWorkCounts,
    ExactStorage,
)> {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(&mut observer, limits, zero_work());
    let cells = if reuse_prevalidated_regions {
        build_overlap_cells(faces, pairs, &mut runtime)
    } else {
        build_overlap_cells_without_prevalidated_region_reuse(faces, pairs, &mut runtime)
    }?;
    Ok((cells, runtime.work, runtime.exact_storage))
}

fn build_cells_with_region_face_candidate_propagation_mode(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    limits: GlobalFlatFoldabilityLimits,
    propagate_region_face_candidates: bool,
) -> FacewiseResult<(
    Vec<OverlapCell>,
    GlobalFlatFoldabilityWorkCounts,
    ExactStorage,
)> {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(&mut observer, limits, zero_work());
    let cells = if propagate_region_face_candidates {
        build_overlap_cells(faces, pairs, &mut runtime)
    } else {
        build_overlap_cells_without_region_face_candidate_propagation(faces, pairs, &mut runtime)
    }?;
    Ok((cells, runtime.work, runtime.exact_storage))
}

fn build_cells_with_global_supporting_line_priority_mode(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    limits: GlobalFlatFoldabilityLimits,
    prioritize_global_supporting_lines: bool,
) -> FacewiseResult<(
    Vec<OverlapCell>,
    GlobalFlatFoldabilityWorkCounts,
    ExactStorage,
)> {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(&mut observer, limits, zero_work());
    let cells = if prioritize_global_supporting_lines {
        build_overlap_cells(faces, pairs, &mut runtime)
    } else {
        build_overlap_cells_without_global_supporting_line_priority(faces, pairs, &mut runtime)
    }?;
    Ok((cells, runtime.work, runtime.exact_storage))
}

fn exact_storage_signature(storage: ExactStorage) -> (usize, usize, usize, usize, usize, usize) {
    (
        storage.embedding_bytes,
        storage.arrangement_bytes,
        storage.snapshot_bytes,
        storage.certificate_structure_bytes,
        storage.verification_bytes,
        storage.constraint_bytes,
    )
}

fn baseline_overlap_cell_interiors_are_disjoint(cells: &[OverlapCell]) -> bool {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    for first_cell in 0..cells.len() {
        for second_cell in (first_cell + 1)..cells.len() {
            let intersection = convex_polygon_intersection(
                &cells[first_cell].boundary,
                &cells[second_cell].boundary,
                &mut runtime,
            )
            .expect("baseline cell intersection");
            if intersection.len() >= 3
                && signed_double_area(&intersection, &mut runtime)
                    .expect("baseline intersection area")
                    .is_positive()
            {
                return false;
            }
        }
    }
    true
}

fn run_single_pass_split(
    polygon: &[Point],
    line_first: &Point,
    line_second: &Point,
) -> (Vec<Point>, Vec<Point>, GlobalFlatFoldabilityWorkCounts) {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let (left, right) =
        split_convex_polygon_by_line(polygon, line_first, line_second, 0, &mut runtime)
            .expect("single-pass exact split");
    (left, right, runtime.work)
}

fn run_dual_clip_split(
    polygon: &[Point],
    line_first: &Point,
    line_second: &Point,
) -> (Vec<Point>, Vec<Point>, GlobalFlatFoldabilityWorkCounts) {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let left = clip_polygon_halfplane(polygon, line_first, line_second, true, 0, &mut runtime)
        .expect("baseline left clip");
    let left_bytes = exact_storage_bytes_points(&left).expect("baseline left exact bytes");
    let right = clip_polygon_halfplane(
        polygon,
        line_first,
        line_second,
        false,
        left_bytes,
        &mut runtime,
    )
    .expect("baseline right clip");
    (left, right, runtime.work)
}

fn assert_single_pass_matches_dual_clip(
    label: &str,
    polygon: &[Point],
    line_first: &Point,
    line_second: &Point,
) -> (Vec<Point>, Vec<Point>, GlobalFlatFoldabilityWorkCounts) {
    let optimized = run_single_pass_split(polygon, line_first, line_second);
    let baseline = run_dual_clip_split(polygon, line_first, line_second);
    assert_eq!(optimized.0, baseline.0, "{label}: left boundary");
    assert_eq!(optimized.1, baseline.1, "{label}: right boundary");
    assert!(
        optimized.2.exact_operations < baseline.2.exact_operations,
        "{label}: the shared side/intersection pass must reduce exact operations"
    );
    assert!(
        optimized.2.exact_values < baseline.2.exact_values,
        "{label}: the shared side/intersection pass must reduce exact values"
    );
    optimized
}

fn assert_certificate_reverification_failed(result: FacewiseResult<()>) {
    assert!(matches!(
        result,
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed
            }
        ))
    ));
}

#[test]
fn taco_taco_compiler_matches_all_fixed_source_rows_after_direction_reversal() {
    let (a, b, c, d) = (3_usize, 0_usize, 2_usize, 1_usize);
    let directed_relations = [(a, b), (c, d), (c, b), (a, d), (a, c), (b, d)];
    let variables = [(0_usize, 1_usize), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let constraint = match relation_constraint(
        RelationConstraintInput {
            kind: FacewiseConstraintKind::TacoTaco,
            relations: &directed_relations,
            faces: &[a, b, c, d],
            supporting_cell: None,
            variable_pairs: &variables,
        },
        taco_taco_source_tuple_accepts,
        &runtime,
        ConstraintStorageScope::Primary,
    ) {
        Ok(constraint) => constraint,
        Err(_) => panic!("six-relation taco-taco constraint compiles"),
    };
    assert_eq!(constraint.variables, vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(constraint.allowed_rows.len(), 16);
    for canonical_row in 0_u8..64 {
        let directed = directed_relations
            .iter()
            .map(|&(first, second)| {
                match directed_face_above_from_row(
                    first,
                    second,
                    canonical_row,
                    &constraint.variables,
                    &variables,
                ) {
                    Ok(value) => value,
                    Err(_) => panic!("every directed pair maps to a canonical variable"),
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            constraint.allowed_rows.contains(&canonical_row),
            taco_taco_source_tuple_accepts(&directed),
            "canonical assignment row {canonical_row:06b}"
        );
    }
}

#[test]
fn source_taco_taco_table_rejects_sum_only_counterexamples() {
    assert!(!taco_taco_source_tuple_accepts(&[
        true, true, false, true, true, false,
    ]));
    assert!(!taco_taco_source_tuple_accepts(&[
        true, true, true, false, false, true,
    ]));
}

#[test]
fn taco_taco_table_is_invariant_under_swapping_each_taco() {
    for canonical_row in 0_u8..64 {
        let mut pair_values = PairValues::default();
        for (position, pair) in [(0_usize, 1_usize), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]
            .into_iter()
            .enumerate()
        {
            pair_values.insert(pair, canonical_row & (1 << position) != 0);
        }
        let tuple = |a, b, c, d| {
            [
                face_above(a, b, &pair_values).unwrap_or(false),
                face_above(c, d, &pair_values).unwrap_or(false),
                face_above(c, b, &pair_values).unwrap_or(false),
                face_above(a, d, &pair_values).unwrap_or(false),
                face_above(a, c, &pair_values).unwrap_or(false),
                face_above(b, d, &pair_values).unwrap_or(false),
            ]
        };
        let expected = taco_taco_source_tuple_accepts(&tuple(0, 1, 2, 3));
        assert_eq!(taco_taco_source_tuple_accepts(&tuple(1, 0, 2, 3)), expected);
        assert_eq!(taco_taco_source_tuple_accepts(&tuple(0, 1, 3, 2)), expected);
    }
}

#[test]
fn two_and_three_relation_templates_match_the_source_truth_tables() {
    let variables = [(0_usize, 1_usize), (0, 2), (1, 2)];
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let transitivity = match relation_constraint(
        RelationConstraintInput {
            kind: FacewiseConstraintKind::Transitivity,
            relations: &[(0, 1), (1, 2), (2, 0)],
            faces: &[0, 1, 2],
            supporting_cell: None,
            variable_pairs: &variables,
        },
        |relations| !(relations[0] == relations[1] && relations[1] == relations[2]),
        &runtime,
        ConstraintStorageScope::Primary,
    ) {
        Ok(value) => value,
        Err(_) => panic!("transitivity compiles"),
    };
    assert_eq!(transitivity.allowed_rows.len(), 6);
    let taco_tortilla = match relation_constraint(
        RelationConstraintInput {
            kind: FacewiseConstraintKind::TacoTortilla,
            relations: &[(0, 2), (1, 2)],
            faces: &[0, 1, 2],
            supporting_cell: None,
            variable_pairs: &variables,
        },
        |relations| relations[0] == relations[1],
        &runtime,
        ConstraintStorageScope::Primary,
    ) {
        Ok(value) => value,
        Err(_) => panic!("taco-tortilla compiles"),
    };
    let tortilla_tortilla = match relation_constraint(
        RelationConstraintInput {
            kind: FacewiseConstraintKind::TortillaTortilla,
            relations: &[(0, 2), (1, 2)],
            faces: &[0, 1, 2],
            supporting_cell: None,
            variable_pairs: &variables,
        },
        |relations| relations[0] == relations[1],
        &runtime,
        ConstraintStorageScope::Primary,
    ) {
        Ok(value) => value,
        Err(_) => panic!("tortilla-tortilla compiles"),
    };
    assert_eq!(taco_tortilla.allowed_rows.len(), 2);
    assert_eq!(taco_tortilla.allowed_rows, tortilla_tortilla.allowed_rows);
}

#[test]
fn tournament_degree_sequence_matches_exhaustive_triangle_transitivity() {
    for vertex_count in 0_usize..=6 {
        let pair_count = choose_two(vertex_count).expect("small exhaustive fixture");
        for mask in 0_u64..(1_u64 << pair_count) {
            let mut above = vec![vec![false; vertex_count]; vertex_count];
            let mut outdegrees = vec![0_usize; vertex_count];
            let pairs = (0..vertex_count)
                .flat_map(|first| ((first + 1)..vertex_count).map(move |second| (first, second)));
            for (pair_index, (first, second)) in pairs.enumerate() {
                let first_above_second = mask & (1_u64 << pair_index) != 0;
                above[first][second] = first_above_second;
                above[second][first] = !first_above_second;
                let winner = if first_above_second { first } else { second };
                outdegrees[winner] += 1;
            }
            let has_directed_triangle = (0..vertex_count).any(|first| {
                ((first + 1)..vertex_count).any(|second| {
                    ((second + 1)..vertex_count).any(|third| {
                        above[first][second] == above[second][third]
                            && above[second][third] == above[third][first]
                    })
                })
            });
            assert_eq!(
                transitive_tournament_degree_sequence(&mut outdegrees),
                !has_directed_triangle,
                "vertex_count={vertex_count}, mask={mask}"
            );
        }
    }
    assert!(!transitive_tournament_degree_sequence(&mut [0, 0, 2]));
    assert!(!transitive_tournament_degree_sequence(&mut [0, 1, 3]));
}

#[test]
fn exact_segment_classification_is_open_and_positive_length_only() {
    let square = vec![
        integer_point(-1, -1),
        integer_point(1, -1),
        integer_point(1, 1),
        integer_point(-1, 1),
    ];
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    assert!(
        segment_overlaps_face_interior(
            &integer_point(-2, 0),
            &integer_point(2, 0),
            &square,
            &mut runtime,
        )
        .unwrap_or(false)
    );
    assert!(
        !segment_overlaps_face_interior(
            &integer_point(-2, 0),
            &integer_point(-1, 0),
            &square,
            &mut runtime,
        )
        .unwrap_or(true)
    );
    assert!(
        !segment_overlaps_face_interior(
            &integer_point(-1, -1),
            &integer_point(1, -1),
            &square,
            &mut runtime,
        )
        .unwrap_or(true)
    );
    assert!(
        segments_overlap_in_positive_length(
            &integer_point(0, 0),
            &integer_point(2, 0),
            &integer_point(1, 0),
            &integer_point(3, 0),
            &mut runtime,
        )
        .unwrap_or(false)
    );
    assert!(
        !segments_overlap_in_positive_length(
            &integer_point(0, 0),
            &integer_point(2, 0),
            &integer_point(2, 0),
            &integer_point(3, 0),
            &mut runtime,
        )
        .unwrap_or(true)
    );
    assert!(
        !segments_overlap_in_positive_length(
            &integer_point(0, 0),
            &integer_point(2, 0),
            &integer_point(1, -1),
            &integer_point(1, 1),
            &mut runtime,
        )
        .unwrap_or(true)
    );
}

#[test]
fn geometry_enumerates_taco_tortilla_and_same_side_taco_taco_only() {
    let upper = vec![
        integer_point(-2, 0),
        integer_point(2, 0),
        integer_point(2, 2),
        integer_point(-2, 2),
    ];
    let crossing = vec![
        integer_point(-1, -1),
        integer_point(1, -1),
        integer_point(1, 1),
        integer_point(-1, 1),
    ];
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let taco_tortilla_embedding = FlatEmbedding {
        reference_face: 0,
        faces: vec![
            synthetic_face(0, upper.clone(), true),
            synthetic_face(1, upper.clone(), false),
            synthetic_face(2, crossing, true),
        ],
        hinges: vec![FoldedHinge {
            edge: fixed_id(0xa01),
            first_face: 0,
            second_face: 1,
            assignment: FoldAssignment::Mountain,
            first_point: integer_point(-1, 0),
            second_point: integer_point(1, 0),
        }],
        material_internal_edge_count: 1,
    };
    let taco_tortilla_cells = vec![synthetic_cell(
        1,
        vec![
            integer_point(-1, 0),
            integer_point(1, 0),
            integer_point(1, 1),
            integer_point(-1, 1),
        ],
        vec![0, 1, 2],
    )];
    let problem = build_constraint_problem(
        &taco_tortilla_embedding,
        &all_pairs(3),
        &taco_tortilla_cells,
        &mut runtime,
        true,
    )
    .expect("hinge crossing a third face builds constraints");
    assert_eq!(
        problem
            .constraints
            .try_iter()
            .expect("constraint iterator allocates")
            .filter(|constraint| constraint.kind() == FacewiseConstraintKind::TacoTortilla)
            .count(),
        1
    );

    let same_side_embedding = FlatEmbedding {
        reference_face: 0,
        faces: vec![
            synthetic_face(0, upper.clone(), true),
            synthetic_face(1, upper.clone(), false),
            synthetic_face(2, upper.clone(), true),
            synthetic_face(3, upper.clone(), false),
        ],
        hinges: vec![
            FoldedHinge {
                edge: fixed_id(0xa11),
                first_face: 0,
                second_face: 1,
                assignment: FoldAssignment::Mountain,
                first_point: integer_point(-1, 0),
                second_point: integer_point(1, 0),
            },
            FoldedHinge {
                edge: fixed_id(0xa12),
                first_face: 2,
                second_face: 3,
                assignment: FoldAssignment::Valley,
                first_point: integer_point(-1, 0),
                second_point: integer_point(1, 0),
            },
        ],
        material_internal_edge_count: 2,
    };
    let same_side_cells = vec![synthetic_cell(2, upper.clone(), vec![0, 1, 2, 3])];
    let same_side = build_constraint_problem(
        &same_side_embedding,
        &all_pairs(4),
        &same_side_cells,
        &mut runtime,
        true,
    )
    .expect("same-side tacos build constraints");
    assert_eq!(
        same_side
            .constraints
            .try_iter()
            .expect("constraint iterator allocates")
            .filter(|constraint| constraint.kind() == FacewiseConstraintKind::TacoTaco)
            .count(),
        1
    );

    let lower = vec![
        integer_point(-2, -2),
        integer_point(2, -2),
        integer_point(2, 0),
        integer_point(-2, 0),
    ];
    let opposite_embedding = FlatEmbedding {
        reference_face: 0,
        faces: vec![
            synthetic_face(0, upper.clone(), true),
            synthetic_face(1, upper.clone(), false),
            synthetic_face(2, lower.clone(), true),
            synthetic_face(3, lower.clone(), false),
        ],
        hinges: same_side_embedding.hinges,
        material_internal_edge_count: 2,
    };
    let opposite_pairs = vec![
        OverlapPair {
            first: 0,
            second: 1,
        },
        OverlapPair {
            first: 2,
            second: 3,
        },
    ];
    let opposite_cells = vec![
        synthetic_cell(3, upper, vec![0, 1]),
        synthetic_cell(4, lower, vec![2, 3]),
    ];
    let opposite = build_constraint_problem(
        &opposite_embedding,
        &opposite_pairs,
        &opposite_cells,
        &mut runtime,
        true,
    )
    .expect("opposite-side tacos remain independently ordered");
    assert_eq!(
        opposite
            .constraints
            .try_iter()
            .expect("constraint iterator allocates")
            .filter(|constraint| constraint.kind() == FacewiseConstraintKind::TacoTaco)
            .count(),
        0
    );
}

#[test]
fn mountain_valley_fixing_covers_orientation_and_hinge_order() {
    assert!(mountain_valley_canonical_value(
        FoldAssignment::Mountain,
        true,
        0,
        1
    ));
    assert!(!mountain_valley_canonical_value(
        FoldAssignment::Valley,
        true,
        0,
        1
    ));
    assert!(!mountain_valley_canonical_value(
        FoldAssignment::Mountain,
        false,
        0,
        1
    ));
    assert!(mountain_valley_canonical_value(
        FoldAssignment::Valley,
        false,
        0,
        1
    ));
    assert!(!mountain_valley_canonical_value(
        FoldAssignment::Mountain,
        true,
        1,
        0
    ));
}

#[test]
fn disjoint_cell_cycle_has_no_global_linearization_but_local_orders_remain_valid() {
    let mut pair_values = PairValues::default();
    pair_values.insert((0, 1), true);
    pair_values.insert((1, 2), true);
    pair_values.insert((0, 2), false);
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    assert_eq!(
        canonical_global_linear_extension(3, &pair_values, &mut runtime)
            .expect("global extension check"),
        None
    );
    for faces in [[0_usize, 1_usize], [1, 2], [0, 2]] {
        assert!(order_cell_faces(&faces, &pair_values, &mut runtime).is_ok());
    }
}

#[test]
fn canonical_cell_reverification_rejects_split_missing_merged_and_duplicate_partitions() {
    let overlay_faces = vec![
        synthetic_face(0, rectangle(0, 0, 4, 4), true),
        synthetic_face(1, rectangle(0, 0, 4, 4), false),
    ];
    let (_, canonical_overlay) = build_test_arrangement(&overlay_faces);
    assert_eq!(canonical_overlay.len(), 1);
    let original = &canonical_overlay[0];

    let split_first = integer_point(1, -1);
    let split_second = integer_point(1, 5);
    let mut split_observer = NoopGlobalFlatFoldabilityObserver;
    let mut split_runtime = Runtime::new(
        &mut split_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let first_boundary = clip_polygon_halfplane(
        &original.boundary,
        &split_first,
        &split_second,
        true,
        0,
        &mut split_runtime,
    )
    .expect("first artificial half");
    let second_boundary = clip_polygon_halfplane(
        &original.boundary,
        &split_first,
        &split_second,
        false,
        exact_storage_bytes_points(&first_boundary).expect("first half bytes"),
        &mut split_runtime,
    )
    .expect("second artificial half");
    let artificially_split = [first_boundary, second_boundary]
        .into_iter()
        .map(|boundary| OverlapCell {
            key: overlap_cell_key(
                &boundary,
                &original.covering_faces,
                &overlay_faces,
                &mut split_runtime,
            )
            .expect("artificial cell key"),
            boundary,
            covering_faces: original.covering_faces.clone(),
        })
        .collect::<Vec<_>>();
    assert_certificate_reverification_failed(verify_canonical_overlap_cells(
        &overlay_faces,
        &artificially_split,
        &mut split_runtime,
    ));

    let mut missing_runtime = Runtime::new(
        &mut split_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    assert_certificate_reverification_failed(verify_canonical_overlap_cells(
        &overlay_faces,
        &[],
        &mut missing_runtime,
    ));

    let mut duplicated = canonical_overlay.clone();
    duplicated.push(canonical_overlay[0].clone());
    let mut duplicate_runtime = Runtime::new(
        &mut split_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    assert_certificate_reverification_failed(verify_canonical_overlap_cells(
        &overlay_faces,
        &duplicated,
        &mut duplicate_runtime,
    ));

    // The remote face contributes x=2 and x=3 supporting lines. They
    // canonically subdivide the identical two-face overlap even though
    // the covering set is unchanged on both sides of each line.
    let merge_faces = vec![
        synthetic_face(0, rectangle(0, 0, 4, 4), true),
        synthetic_face(1, rectangle(0, 0, 4, 4), false),
        synthetic_face(2, rectangle(2, 6, 3, 7), true),
    ];
    let (_, canonical_merge) = build_test_arrangement(&merge_faces);
    assert_eq!(
        canonical_merge
            .iter()
            .filter(|cell| cell.covering_faces == [0, 1])
            .count(),
        3
    );
    let mut merged = canonical_merge
        .iter()
        .filter(|cell| cell.covering_faces != [0, 1])
        .cloned()
        .collect::<Vec<_>>();
    let merged_boundary = rectangle(0, 0, 4, 4);
    let mut merged_runtime = Runtime::new(
        &mut split_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    merged.push(OverlapCell {
        key: overlap_cell_key(&merged_boundary, &[0, 1], &merge_faces, &mut merged_runtime)
            .expect("merged cell key"),
        boundary: merged_boundary,
        covering_faces: vec![0, 1],
    });
    assert_certificate_reverification_failed(verify_canonical_overlap_cells(
        &merge_faces,
        &merged,
        &mut merged_runtime,
    ));

    let mut canonical_runtime = Runtime::new(
        &mut split_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    verify_canonical_overlap_cells(&merge_faces, &canonical_merge, &mut canonical_runtime)
        .expect("canonical arrangement reverifies");
    let mut reverse_stored = canonical_merge.clone();
    reverse_stored.reverse();
    verify_canonical_overlap_cells(&merge_faces, &reverse_stored, &mut canonical_runtime)
        .expect("cell storage order is not geometric evidence");
}

#[test]
fn verified_cell_partition_rejects_missing_three_face_common_interior() {
    for faces in [
        vec![
            synthetic_face(0, rectangle(0, 0, 4, 4), true),
            synthetic_face(1, rectangle(0, 0, 4, 4), false),
            synthetic_face(2, rectangle(0, 0, 4, 4), true),
        ],
        vec![
            synthetic_face(0, rectangle(0, 0, 6, 4), true),
            synthetic_face(1, rectangle(1, 0, 5, 4), false),
            synthetic_face(2, rectangle(2, 0, 4, 4), true),
        ],
    ] {
        let (_, canonical) = build_test_arrangement(&faces);
        assert!(
            canonical
                .iter()
                .any(|cell| cell.covering_faces == [0, 1, 2]),
            "the fixture has a positive three-face common interior",
        );
        let mut tampered = canonical.clone();
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        for cell in &mut tampered {
            if cell.covering_faces == [0, 1, 2] {
                cell.covering_faces.pop();
                cell.key =
                    overlap_cell_key(&cell.boundary, &cell.covering_faces, &faces, &mut runtime)
                        .expect("tampered cell key");
            }
        }
        assert_certificate_reverification_failed(verify_canonical_overlap_cells(
            &faces,
            &tampered,
            &mut runtime,
        ));
    }
}

#[test]
fn supporting_line_deduplication_preserves_canonical_cells_and_keys() {
    let faces = vec![
        synthetic_face(0, rectangle(0, 0, 4, 4), true),
        synthetic_face(1, rectangle(0, 0, 4, 4), false),
        synthetic_face(2, rectangle(2, 6, 3, 7), true),
        synthetic_face(3, rectangle(2, 6, 3, 7), false),
        synthetic_face(4, rectangle(4, 1, 6, 3), true),
    ];
    let mut pair_observer = NoopGlobalFlatFoldabilityObserver;
    let mut pair_runtime = Runtime::new(
        &mut pair_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let pairs = build_overlap_pairs(&faces, &mut pair_runtime).expect("fixture overlap pairs");
    let (optimized, optimized_work, optimized_storage) =
        build_cells_with_supporting_line_deduplication_mode(
            &faces,
            &pairs,
            GlobalFlatFoldabilityLimits::default(),
            true,
        )
        .expect("deduplicated-line arrangement");
    let (baseline, baseline_work, baseline_storage) =
        build_cells_with_supporting_line_deduplication_mode(
            &faces,
            &pairs,
            GlobalFlatFoldabilityLimits::default(),
            false,
        )
        .expect("all-line arrangement");

    assert_eq!(
        overlap_cell_signatures(&optimized),
        overlap_cell_signatures(&baseline)
    );
    assert_eq!(
        optimized_work.arrangement_segments,
        baseline_work.arrangement_segments
    );
    assert_eq!(optimized_work.overlap_cells, baseline_work.overlap_cells);
    assert_eq!(optimized_storage.total(), baseline_storage.total());
    assert!(optimized_work.exact_operations < baseline_work.exact_operations);
    assert!(optimized_work.exact_values < baseline_work.exact_values);
}

#[test]
fn prevalidated_region_reuse_preserves_remote_line_cells_and_exact_limits() {
    let faces = vec![
        synthetic_face(0, rectangle(0, 0, 8, 8), true),
        synthetic_face(1, rectangle(0, 0, 8, 8), false),
        // These disjoint faces contribute supporting lines that repeatedly
        // leave prior-generation regions strictly on one side.
        synthetic_face(2, rectangle(2, 12, 3, 13), true),
        synthetic_face(3, rectangle(5, -5, 6, -4), false),
    ];
    let mut pair_observer = NoopGlobalFlatFoldabilityObserver;
    let mut pair_runtime = Runtime::new(
        &mut pair_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let pairs = build_overlap_pairs(&faces, &mut pair_runtime).expect("fixture overlap pairs");
    assert_eq!(
        pairs
            .iter()
            .map(|pair| (pair.first, pair.second))
            .collect::<Vec<_>>(),
        [(0, 1)]
    );

    let (optimized, optimized_work, optimized_storage) =
        build_cells_with_prevalidated_region_reuse_mode(
            &faces,
            &pairs,
            GlobalFlatFoldabilityLimits::default(),
            true,
        )
        .expect("prevalidated region reuse");
    let (baseline, baseline_work, baseline_storage) =
        build_cells_with_prevalidated_region_reuse_mode(
            &faces,
            &pairs,
            GlobalFlatFoldabilityLimits::default(),
            false,
        )
        .expect("revalidated region baseline");

    assert_eq!(
        overlap_cell_signatures(&optimized),
        overlap_cell_signatures(&baseline)
    );
    assert_eq!(
        optimized_work.arrangement_segments,
        baseline_work.arrangement_segments
    );
    assert_eq!(optimized_work.overlap_cells, baseline_work.overlap_cells);
    assert_eq!(optimized_storage.total(), baseline_storage.total());
    assert!(optimized_work.exact_operations < baseline_work.exact_operations);
    assert!(optimized_work.exact_values < baseline_work.exact_values);

    let measured_operations = optimized_work.exact_operations;
    assert!(measured_operations > 0);
    for maximum in [measured_operations, measured_operations - 1] {
        let result = build_cells_with_prevalidated_region_reuse_mode(
            &faces,
            &pairs,
            GlobalFlatFoldabilityLimits {
                max_exact_operations: maximum,
                ..GlobalFlatFoldabilityLimits::default()
            },
            true,
        );
        if maximum == measured_operations {
            let (limited, limited_work, limited_storage) =
                result.expect("exact measured operation limit is admitted");
            assert_eq!(
                overlap_cell_signatures(&limited),
                overlap_cell_signatures(&optimized)
            );
            assert_eq!(limited_work.exact_operations, measured_operations);
            assert_eq!(limited_storage.total(), optimized_storage.total());
        } else {
            assert!(matches!(
                result,
                Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                        resource: FlatFoldabilityResource::ExactOperations,
                        limit,
                        observed,
                    }
                )) if limit == maximum && observed == measured_operations
            ));
        }
    }

    let default_limits = GlobalFlatFoldabilityLimits::default();
    let mut first_success = 0_usize;
    let mut one_past_last_failure = default_limits.max_certificate_bytes;
    while first_success < one_past_last_failure {
        let candidate = first_success + (one_past_last_failure - first_success) / 2;
        let result = build_cells_with_region_face_candidate_propagation_mode(
            &faces,
            &pairs,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: candidate,
                ..default_limits
            },
            true,
        );
        if result.is_ok() {
            one_past_last_failure = candidate;
        } else {
            first_success = candidate + 1;
        }
    }
    let measured_storage_limit = first_success;
    assert!(measured_storage_limit > 0);
    let (limited, _, limited_storage) = build_cells_with_region_face_candidate_propagation_mode(
        &faces,
        &pairs,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: measured_storage_limit,
            ..default_limits
        },
        true,
    )
    .expect("candidate metadata peak fits at exact byte equality");
    assert_eq!(
        overlap_cell_signatures(&limited),
        overlap_cell_signatures(&optimized)
    );
    assert_eq!(limited_storage.total(), optimized_storage.total());
    assert!(matches!(
        build_cells_with_region_face_candidate_propagation_mode(
            &faces,
            &pairs,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: measured_storage_limit - 1,
                ..default_limits
            },
            true,
        ),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit,
                observed,
            }
        )) if limit == measured_storage_limit - 1 && observed == measured_storage_limit
    ));
}

#[test]
fn single_pass_split_matches_dual_clip_for_every_exact_sign_transition() {
    let line_first = integer_point(-4, 0);
    let line_second = integer_point(4, 0);
    for previous_y in [-1_i64, 0, 1] {
        for current_y in [-1_i64, 0, 1] {
            // The cyclic first edge is `polygon.last() -> polygon.first()`, so
            // this two-point fixture directly fixes each of the nine exact
            // previous/current sign transitions, including zero on either end.
            let polygon = vec![integer_point(1, current_y), integer_point(-1, previous_y)];
            assert_single_pass_matches_dual_clip(
                &format!("sign transition {previous_y}->{current_y}"),
                &polygon,
                &line_first,
                &line_second,
            );
        }
    }
}

#[test]
fn single_pass_split_preserves_wrap_contacts_collinearity_and_line_direction() {
    let line_first = integer_point(-5, 0);
    let line_second = integer_point(5, 0);
    let last_zero_wrap = vec![
        integer_point(0, 3),
        integer_point(3, -2),
        integer_point(-3, 0),
    ];
    let (_, wrap_right, _) = assert_single_pass_matches_dual_clip(
        "last-zero wrap",
        &last_zero_wrap,
        &line_first,
        &line_second,
    );
    assert_eq!(
        wrap_right.first(),
        last_zero_wrap.last(),
        "a zero previous vertex at the cyclic wrap must keep the baseline vector start"
    );

    let fixtures = [
        (
            "line through two vertices",
            vec![
                integer_point(0, -3),
                integer_point(3, 0),
                integer_point(0, 3),
                integer_point(-3, 0),
            ],
        ),
        (
            "collinear boundary edge",
            vec![
                integer_point(-3, 0),
                integer_point(3, 0),
                integer_point(3, 2),
                integer_point(-3, 2),
            ],
        ),
        (
            "all vertices on the line",
            vec![
                integer_point(-3, 0),
                integer_point(0, 0),
                integer_point(3, 0),
            ],
        ),
        (
            "fully on the left",
            vec![
                integer_point(-3, 2),
                integer_point(3, 2),
                integer_point(3, 4),
                integer_point(-3, 4),
            ],
        ),
    ];
    for (label, polygon) in fixtures {
        assert_single_pass_matches_dual_clip(label, &polygon, &line_first, &line_second);
    }

    let strict_crossing = rectangle(-4, -3, 5, 3);
    let (forward_left, forward_right, _) = assert_single_pass_matches_dual_clip(
        "forward strict crossing",
        &strict_crossing,
        &line_first,
        &line_second,
    );
    let (reverse_left, reverse_right, _) = assert_single_pass_matches_dual_clip(
        "reversed strict crossing",
        &strict_crossing,
        &line_second,
        &line_first,
    );
    assert_eq!(forward_left, reverse_right);
    assert_eq!(forward_right, reverse_left);
}

#[test]
fn owned_split_reuses_only_strictly_one_sided_inputs_and_preserves_exact_outputs() {
    let line_first = integer_point(-5, 0);
    let line_second = integer_point(5, 0);
    for (label, polygon, expected_reuse) in [
        (
            "strict left",
            rectangle(-4, 2, 5, 5),
            ReusedSplitInput::Left,
        ),
        (
            "strict right",
            rectangle(-4, -5, 5, -2),
            ReusedSplitInput::Right,
        ),
        (
            "boundary touch",
            rectangle(-4, 0, 5, 3),
            ReusedSplitInput::None,
        ),
        (
            "strict crossing",
            rectangle(-4, -3, 5, 3),
            ReusedSplitInput::None,
        ),
    ] {
        let (expected_left, expected_right, baseline_work) =
            run_single_pass_split(&polygon, &line_first, &line_second);
        let input_pointer = polygon.as_ptr();
        let input_capacity = polygon.capacity();
        let input_exact_bytes =
            exact_storage_bytes_points(&polygon).expect("owned input exact bytes");
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        runtime
            .set_arrangement_exact_storage(input_exact_bytes)
            .expect("owned input arrangement fits");
        let (left, right, reused) =
            split_owned_convex_polygon_by_line(polygon, &line_first, &line_second, 0, &mut runtime)
                .expect("owned exact split");
        assert_eq!(left, expected_left, "{label}: left boundary");
        assert_eq!(right, expected_right, "{label}: right boundary");
        assert_eq!(reused, expected_reuse, "{label}: reuse classification");
        assert_eq!(runtime.exact_storage.arrangement_bytes, input_exact_bytes);
        if reused != ReusedSplitInput::None {
            assert!(runtime.work.exact_operations < baseline_work.exact_operations);
            assert!(runtime.work.exact_values < baseline_work.exact_values);
            let reused_output = if reused == ReusedSplitInput::Left {
                &left
            } else {
                &right
            };
            assert_eq!(reused_output.as_ptr(), input_pointer);
            assert_eq!(reused_output.capacity(), input_capacity);
        }
    }
}

#[test]
fn supporting_line_face_masks_track_positive_area_sides_and_reverse_exactly() {
    let faces = vec![
        synthetic_face(0, rectangle(-4, 2, 4, 5), true),
        synthetic_face(1, rectangle(-4, -5, 4, -2), false),
        synthetic_face(2, rectangle(-4, -2, 4, 2), true),
        synthetic_face(3, rectangle(-4, 0, 4, 3), false),
        synthetic_face(
            4,
            vec![
                integer_point(0, 0),
                integer_point(2, 2),
                integer_point(-2, 2),
            ],
            true,
        ),
    ];
    let first = integer_point(-6, 0);
    let second = integer_point(6, 0);
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let (forward, forward_bytes) =
        supporting_line_face_interior_masks(&faces, &first, &second, 0, &mut runtime)
            .expect("forward exact face masks");
    assert_eq!(
        forward,
        [
            FACE_INTERIOR_LEFT,
            FACE_INTERIOR_RIGHT,
            FACE_INTERIOR_LEFT | FACE_INTERIOR_RIGHT,
            FACE_INTERIOR_LEFT,
            FACE_INTERIOR_LEFT,
        ]
    );
    assert_eq!(forward_bytes, forward.capacity());

    let (reverse, reverse_bytes) =
        supporting_line_face_interior_masks(&faces, &second, &first, 0, &mut runtime)
            .expect("reversed exact face masks");
    assert_eq!(
        reverse,
        [
            FACE_INTERIOR_RIGHT,
            FACE_INTERIOR_LEFT,
            FACE_INTERIOR_LEFT | FACE_INTERIOR_RIGHT,
            FACE_INTERIOR_RIGHT,
            FACE_INTERIOR_RIGHT,
        ]
    );
    assert_eq!(reverse_bytes, reverse.capacity());
}

#[test]
fn single_pass_split_accounts_for_both_live_outputs_at_the_exact_peak() {
    let polygon = rectangle(-4, -3, 5, 3);
    let line_first = integer_point(-5, 0);
    let line_second = integer_point(5, 0);
    let (left, right, optimized_work) = assert_single_pass_matches_dual_clip(
        "strict storage crossing",
        &polygon,
        &line_first,
        &line_second,
    );
    let retained_polygon_bytes =
        exact_storage_bytes_points(&polygon).expect("retained polygon exact bytes");
    let output_bytes = exact_storage_bytes_points(&left)
        .expect("left output exact bytes")
        .saturating_add(exact_storage_bytes_points(&right).expect("right output exact bytes"));
    let prior_output_bytes = 17_usize;
    let exact_peak = retained_polygon_bytes
        .saturating_add(prior_output_bytes)
        .saturating_add(output_bytes);

    for maximum in [exact_peak, exact_peak - 1] {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: maximum,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        runtime
            .set_arrangement_exact_storage(retained_polygon_bytes)
            .expect("the retained input polygon fits below the split peak");
        let result = split_convex_polygon_by_line(
            &polygon,
            &line_first,
            &line_second,
            prior_output_bytes,
            &mut runtime,
        );
        if maximum == exact_peak {
            assert_eq!(
                result.expect("both outputs fit at exact equality"),
                (left.clone(), right.clone())
            );
            assert_eq!(
                runtime.work.exact_operations,
                optimized_work.exact_operations
            );
        } else {
            assert!(matches!(
                result,
                Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                        resource: FlatFoldabilityResource::CertificateBytes,
                        limit,
                        observed,
                    }
                )) if limit == maximum && observed == exact_peak
            ));
        }
        assert_eq!(
            runtime.exact_storage.arrangement_bytes, retained_polygon_bytes,
            "the split owns only transient output storage"
        );
    }

    let exact_operations = optimized_work.exact_operations;
    for maximum in [exact_operations, exact_operations - 1] {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_exact_operations: maximum,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        runtime
            .set_arrangement_exact_storage(retained_polygon_bytes)
            .expect("the retained input polygon fits");
        let result = split_convex_polygon_by_line(
            &polygon,
            &line_first,
            &line_second,
            prior_output_bytes,
            &mut runtime,
        );
        if maximum == exact_operations {
            assert_eq!(
                result.expect("single-pass exact work equality is admitted"),
                (left.clone(), right.clone())
            );
        } else {
            assert!(matches!(
                result,
                Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                        resource: FlatFoldabilityResource::ExactOperations,
                        limit,
                        observed,
                    }
                )) if limit == maximum && observed == exact_operations
            ));
        }
        assert_eq!(
            runtime.exact_storage.arrangement_bytes,
            retained_polygon_bytes
        );
    }
}

#[test]
fn exact_bounds_pair_pruning_preserves_canonical_pairs_and_reduces_exact_work() {
    let faces = vec![
        synthetic_face(0, rectangle(0, 0, 4, 4), true),
        synthetic_face(1, rectangle(1, 1, 3, 3), false),
        synthetic_face(2, rectangle(4, 1, 6, 3), true),
        synthetic_face(3, rectangle(4, 4, 6, 6), false),
        synthetic_face(4, rectangle(20, 20, 24, 24), true),
        synthetic_face(5, rectangle(-24, -24, -20, -20), false),
    ];
    let run = |prune| {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        let pairs = if prune {
            build_overlap_pairs(&faces, &mut runtime)
        } else {
            build_overlap_pairs_without_exact_bounds_pruning(&faces, &mut runtime)
        }
        .expect("bounded pair construction");
        (
            pairs
                .into_iter()
                .map(|pair| (pair.first, pair.second))
                .collect::<Vec<_>>(),
            runtime.work,
            runtime.exact_storage,
        )
    };
    let (optimized, optimized_work, optimized_storage) = run(true);
    let (baseline, baseline_work, baseline_storage) = run(false);

    assert_eq!(optimized, baseline);
    assert_eq!(optimized, vec![(0, 1)]);
    assert_eq!(
        optimized_work.overlap_face_pairs,
        baseline_work.overlap_face_pairs
    );
    assert_eq!(optimized_storage.total(), baseline_storage.total());
    assert!(optimized_work.exact_operations < baseline_work.exact_operations);
    assert!(optimized_work.exact_values < baseline_work.exact_values);
}

#[test]
fn region_face_bounds_pruning_preserves_cells_contacts_and_reduces_exact_work() {
    let faces = vec![
        synthetic_face(0, rectangle(0, 0, 8, 8), true),
        synthetic_face(1, rectangle(2, 2, 6, 6), false),
        // These two faces touch the first face at an edge and a point. Strict
        // bounds pruning must retain both boundary cases for the exact
        // representative-point coverage predicate.
        synthetic_face(2, rectangle(8, 1, 10, 3), true),
        synthetic_face(3, rectangle(8, 8, 10, 10), false),
        // Widely separated faces make the saved exact coverage work
        // observable without changing the canonical arrangement.
        synthetic_face(4, rectangle(30, 0, 34, 4), true),
        synthetic_face(5, rectangle(-34, -4, -30, 0), false),
        synthetic_face(6, rectangle(0, 30, 4, 34), true),
        synthetic_face(7, rectangle(-4, -34, 0, -30), false),
    ];
    let mut pair_observer = NoopGlobalFlatFoldabilityObserver;
    let mut pair_runtime = Runtime::new(
        &mut pair_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let pairs = build_overlap_pairs(&faces, &mut pair_runtime).expect("bounded overlap pairs");
    assert_eq!(
        pairs
            .iter()
            .map(|pair| (pair.first, pair.second))
            .collect::<Vec<_>>(),
        [(0, 1)],
        "edge and point contact remain zero-area rather than overlap pairs"
    );

    let (optimized, optimized_work, optimized_storage) =
        build_cells_with_region_face_bounds_pruning_mode(&faces, &pairs, true)
            .expect("bounds-pruned canonical cells");
    let (propagation_baseline, propagation_baseline_work, propagation_baseline_storage) =
        build_cells_with_region_face_candidate_propagation_mode(
            &faces,
            &pairs,
            GlobalFlatFoldabilityLimits::default(),
            false,
        )
        .expect("classification-only bounds baseline");
    let (baseline, baseline_work, baseline_storage) =
        build_cells_with_region_face_bounds_pruning_mode(&faces, &pairs, false)
            .expect("unpruned canonical cells");

    assert_eq!(
        overlap_cell_signatures(&optimized),
        overlap_cell_signatures(&baseline),
        "bounds pruning preserves every key, exact boundary, and covering list"
    );
    assert_eq!(
        overlap_cell_signatures(&optimized),
        overlap_cell_signatures(&propagation_baseline),
        "positive-area face-candidate propagation preserves canonical cells"
    );
    assert!(
        optimized
            .iter()
            .any(|cell| cell.covering_faces.as_slice() == [0, 1])
    );
    for contact_or_separated_face in 2..faces.len() {
        assert!(
            optimized
                .iter()
                .any(|cell| { cell.covering_faces.as_slice() == [contact_or_separated_face] })
        );
    }
    assert!(optimized.iter().all(|cell| {
        !cell.covering_faces.contains(&2) || cell.covering_faces.as_slice() == [2]
    }));
    assert!(optimized.iter().all(|cell| {
        !cell.covering_faces.contains(&3) || cell.covering_faces.as_slice() == [3]
    }));
    assert!(optimized_work.arrangement_segments < propagation_baseline_work.arrangement_segments);
    assert_eq!(
        optimized_work.arrangement_segments,
        baseline_work.arrangement_segments
    );
    assert_eq!(optimized_work.overlap_cells, baseline_work.overlap_cells);
    assert_eq!(optimized_storage.total(), baseline_storage.total());
    assert_eq!(
        optimized_storage.total(),
        propagation_baseline_storage.total()
    );
    assert!(optimized_work.exact_operations < propagation_baseline_work.exact_operations);
    assert!(optimized_work.exact_values < propagation_baseline_work.exact_values);
}

#[test]
fn propagated_face_candidates_exactly_classify_multiway_and_contact_regions() {
    let faces = vec![
        synthetic_face(0, rectangle(0, 0, 8, 8), true),
        synthetic_face(1, rectangle(2, 2, 6, 6), false),
        synthetic_face(2, rectangle(3, 3, 5, 5), true),
        // These contribute an oppositely directed shared supporting line plus
        // edge-only and point-only contacts with face zero.
        synthetic_face(3, rectangle(8, 1, 10, 3), false),
        synthetic_face(4, rectangle(8, 8, 10, 10), true),
        synthetic_face(5, rectangle(-12, -3, -10, -1), false),
    ];
    let mut pair_observer = NoopGlobalFlatFoldabilityObserver;
    let mut pair_runtime = Runtime::new(
        &mut pair_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let pairs = build_overlap_pairs(&faces, &mut pair_runtime).expect("fixture overlap pairs");
    assert_eq!(
        pairs
            .iter()
            .map(|pair| (pair.first, pair.second))
            .collect::<Vec<_>>(),
        [(0, 1), (0, 2), (1, 2)]
    );

    let (propagated, propagated_work, propagated_storage) =
        build_cells_with_region_face_candidate_propagation_mode(
            &faces,
            &pairs,
            GlobalFlatFoldabilityLimits::default(),
            true,
        )
        .expect("propagated exact classification");
    let (canonical_order, canonical_order_work, canonical_order_storage) =
        build_cells_with_global_supporting_line_priority_mode(
            &faces,
            &pairs,
            GlobalFlatFoldabilityLimits::default(),
            false,
        )
        .expect("prior canonical supporting-line order");
    let (independent, independent_work, independent_storage) =
        build_cells_with_region_face_candidate_propagation_mode(
            &faces,
            &pairs,
            GlobalFlatFoldabilityLimits::default(),
            false,
        )
        .expect("independent representative-point classification");

    assert_eq!(
        overlap_cell_signatures(&propagated),
        overlap_cell_signatures(&independent)
    );
    assert_eq!(
        overlap_cell_signatures(&propagated),
        overlap_cell_signatures(&canonical_order)
    );
    for expected_covering_faces in [[0].as_slice(), &[0, 1], &[0, 1, 2], &[3], &[4], &[5]] {
        assert!(
            propagated
                .iter()
                .any(|cell| cell.covering_faces == expected_covering_faces)
        );
    }
    assert!(propagated_work.arrangement_segments < independent_work.arrangement_segments);
    assert_eq!(
        propagated_work.overlap_cells,
        independent_work.overlap_cells
    );
    assert_eq!(
        exact_storage_signature(propagated_storage),
        exact_storage_signature(independent_storage)
    );
    assert_eq!(
        exact_storage_signature(propagated_storage),
        exact_storage_signature(canonical_order_storage)
    );
    assert!(propagated_work.exact_operations < canonical_order_work.exact_operations);
    assert!(propagated_work.exact_values < canonical_order_work.exact_values);
    assert!(propagated_work.exact_operations < independent_work.exact_operations);
    assert!(propagated_work.exact_values < independent_work.exact_values);

    let measured_operations = propagated_work.exact_operations;
    assert!(measured_operations > 0);
    for maximum in [measured_operations, measured_operations - 1] {
        let result = build_cells_with_global_supporting_line_priority_mode(
            &faces,
            &pairs,
            GlobalFlatFoldabilityLimits {
                max_exact_operations: maximum,
                ..GlobalFlatFoldabilityLimits::default()
            },
            true,
        );
        if maximum == measured_operations {
            let (limited, limited_work, limited_storage) =
                result.expect("global-line priority fits exact operation equality");
            assert_eq!(
                overlap_cell_signatures(&limited),
                overlap_cell_signatures(&propagated)
            );
            assert_eq!(limited_work.exact_operations, measured_operations);
            assert_eq!(
                exact_storage_signature(limited_storage),
                exact_storage_signature(propagated_storage)
            );
        } else {
            assert!(matches!(
                result,
                Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                        resource: FlatFoldabilityResource::ExactOperations,
                        limit,
                        observed,
                    }
                )) if limit == maximum && observed == measured_operations
            ));
        }
    }
}

#[test]
fn region_face_candidate_scope_restores_saved_arrangement_on_control_abort() {
    let faces = vec![
        synthetic_face(0, rectangle(0, 0, 8, 8), true),
        synthetic_face(1, rectangle(2, 2, 6, 6), false),
        synthetic_face(2, rectangle(12, 0, 14, 2), true),
        synthetic_face(3, rectangle(-14, -2, -12, 0), false),
        synthetic_face(4, rectangle(0, 12, 2, 14), true),
        synthetic_face(5, rectangle(-2, -14, 0, -12), false),
    ];
    let mut pair_observer = NoopGlobalFlatFoldabilityObserver;
    let mut pair_runtime = Runtime::new(
        &mut pair_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let pairs = build_overlap_pairs(&faces, &mut pair_runtime).expect("fixture overlap pairs");
    let saved_arrangement_bytes = 17_usize;

    let mut deadline_observer = DeadlineAfter {
        continued_checkpoints: 20,
    };
    let mut deadline_runtime = Runtime::new(
        &mut deadline_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    deadline_runtime
        .set_arrangement_exact_storage(saved_arrangement_bytes)
        .expect("saved deadline arrangement");
    assert!(matches!(
        build_overlap_cells(&faces, &pairs, &mut deadline_runtime),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. }
        ))
    ));
    assert_eq!(
        deadline_runtime.exact_storage.arrangement_bytes,
        saved_arrangement_bytes
    );

    let mut cancel_observer = CancelAfter {
        continued_checkpoints: 20,
    };
    let mut cancel_runtime = Runtime::new(
        &mut cancel_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    cancel_runtime
        .set_arrangement_exact_storage(saved_arrangement_bytes)
        .expect("saved cancel arrangement");
    assert!(matches!(
        build_overlap_cells(&faces, &pairs, &mut cancel_runtime),
        Err(FacewiseAbort::Execution(
            GlobalFlatFoldabilityExecutionError::Cancelled
        ))
    ));
    assert_eq!(
        cancel_runtime.exact_storage.arrangement_bytes,
        saved_arrangement_bytes
    );
}

#[test]
fn arrangement_face_bounds_scope_accounts_exact_storage_and_restores_control_failures() {
    let faces = vec![
        synthetic_face(0, rectangle(-10, -1, -8, 1), true),
        synthetic_face(1, rectangle(8, -1, 10, 1), false),
    ];
    let retained_region_bytes =
        exact_storage_bytes_points(&rectangle(-1, -1, 1, 1)).expect("retained region bytes");

    let mut measuring_observer = NoopGlobalFlatFoldabilityObserver;
    let mut measuring_runtime = Runtime::new(
        &mut measuring_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    measuring_runtime
        .set_arrangement_exact_storage(retained_region_bytes)
        .expect("retained region fits");
    let measured_bounds =
        build_arrangement_face_bounds(&faces, retained_region_bytes, &mut measuring_runtime)
            .expect("measure retained face bounds");
    let retained_bounds_bytes = measuring_runtime
        .exact_storage
        .arrangement_bytes
        .checked_sub(retained_region_bytes)
        .expect("bounds extend the retained region scope");
    assert_eq!(
        retained_bounds_bytes,
        measured_bounds.capacity() * std::mem::size_of::<ExactAxisAlignedBounds>()
    );
    drop(measured_bounds);

    let embedding_bytes = 19_usize;
    let exact_peak = embedding_bytes
        .checked_add(retained_region_bytes)
        .and_then(|total| total.checked_add(retained_bounds_bytes))
        .expect("small fixture peak");
    for maximum in [exact_peak, exact_peak - 1] {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: maximum,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        runtime
            .set_embedding_exact_storage(embedding_bytes)
            .expect("preexisting embedding fits");
        runtime
            .set_arrangement_exact_storage(retained_region_bytes)
            .expect("preexisting region fits");
        let result = build_arrangement_face_bounds(&faces, retained_region_bytes, &mut runtime);
        if maximum == exact_peak {
            let bounds = result.expect("retained bounds fit at exact equality");
            assert_eq!(bounds.len(), faces.len());
            assert_eq!(runtime.exact_storage.total(), Some(exact_peak));
            drop(bounds);
        } else {
            assert!(matches!(
                result,
                Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                        resource: FlatFoldabilityResource::CertificateBytes,
                        limit,
                        observed,
                    }
                )) if limit == maximum && observed == exact_peak
            ));
            assert_eq!(
                runtime.exact_storage.arrangement_bytes, retained_region_bytes,
                "failed bounds admission restores the entry arrangement scope"
            );
        }
    }

    let mut deadline_observer = DeadlineAfter {
        continued_checkpoints: 0,
    };
    let mut deadline_runtime = Runtime::new(
        &mut deadline_observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: retained_region_bytes,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    deadline_runtime
        .set_arrangement_exact_storage(retained_region_bytes)
        .expect("deadline entry region fits");
    assert!(matches!(
        build_arrangement_face_bounds(&faces, retained_region_bytes, &mut deadline_runtime,),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. }
        ))
    ));
    assert_eq!(
        deadline_runtime.exact_storage.arrangement_bytes,
        retained_region_bytes
    );

    let mut cancel_observer = CancelAfter {
        continued_checkpoints: 1,
    };
    let mut cancel_runtime = Runtime::new(
        &mut cancel_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    cancel_runtime
        .set_arrangement_exact_storage(retained_region_bytes)
        .expect("cancel entry region fits");
    assert!(matches!(
        build_arrangement_face_bounds(&faces, retained_region_bytes, &mut cancel_runtime),
        Err(FacewiseAbort::Execution(
            GlobalFlatFoldabilityExecutionError::Cancelled
        ))
    ));
    assert_eq!(
        cancel_runtime.exact_storage.arrangement_bytes,
        retained_region_bytes
    );
}

#[test]
fn exact_bounds_pair_pruning_accounts_retained_storage_at_the_exact_boundary() {
    let faces = vec![
        synthetic_face(0, rectangle(0, 0, 2, 2), true),
        synthetic_face(1, rectangle(3, 0, 5, 2), false),
    ];
    let mut capacity_probe = Vec::<ExactAxisAlignedBounds>::new();
    capacity_probe
        .try_reserve_exact(faces.len())
        .expect("small exact-bounds capacity probe");
    let bounds_bytes = capacity_probe.capacity() * std::mem::size_of::<ExactAxisAlignedBounds>();

    for maximum in [bounds_bytes, bounds_bytes - 1] {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: maximum,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        let result = build_overlap_pairs(&faces, &mut runtime);
        if maximum == bounds_bytes {
            assert!(
                result
                    .expect("exact retained bounds storage is admitted")
                    .is_empty()
            );
        } else {
            assert!(matches!(
                result,
                Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                        resource: FlatFoldabilityResource::CertificateBytes,
                        limit,
                        observed,
                    }
                )) if limit == maximum && observed == bounds_bytes
            ));
        }
        assert_eq!(runtime.exact_storage.arrangement_bytes, 0);
    }
}

#[test]
fn verifier_face_bounds_scope_accounts_exact_storage_and_restores_control_failures() {
    let faces = vec![
        synthetic_face(0, rectangle(-5, -2, -1, 2), true),
        synthetic_face(1, rectangle(1, -2, 5, 2), false),
    ];
    let embedding_bytes = 13_usize;
    let arrangement_bytes = 17_usize;
    let verification_base = 23_usize;

    let mut measuring_observer = NoopGlobalFlatFoldabilityObserver;
    let mut measuring_runtime = Runtime::new(
        &mut measuring_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    measuring_runtime
        .add_verification_storage(verification_base)
        .expect("verification base fits");
    let measured_bounds = build_verifier_face_bounds(&faces, &mut measuring_runtime)
        .expect("measure verifier face bounds");
    let retained_bounds_bytes = measuring_runtime
        .verification_storage_bytes()
        .checked_sub(verification_base)
        .expect("face bounds extend verification storage");
    assert_eq!(
        retained_bounds_bytes,
        measured_bounds.capacity() * std::mem::size_of::<ExactAxisAlignedBounds>()
    );
    drop(measured_bounds);

    let exact_peak = embedding_bytes
        .checked_add(arrangement_bytes)
        .and_then(|total| total.checked_add(verification_base))
        .and_then(|total| total.checked_add(retained_bounds_bytes))
        .expect("small verifier peak");
    for maximum in [exact_peak, exact_peak - 1] {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: maximum,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        runtime
            .set_embedding_exact_storage(embedding_bytes)
            .expect("embedding fits");
        runtime
            .set_arrangement_exact_storage(arrangement_bytes)
            .expect("arrangement fits");
        runtime
            .add_verification_storage(verification_base)
            .expect("verification base fits");
        let result = build_verifier_face_bounds(&faces, &mut runtime);
        if maximum == exact_peak {
            let bounds = result.expect("verifier bounds fit at exact equality");
            assert_eq!(bounds.len(), faces.len());
            assert_eq!(runtime.exact_storage.total(), Some(exact_peak));
            drop(bounds);
        } else {
            assert!(matches!(
                result,
                Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                        resource: FlatFoldabilityResource::CertificateBytes,
                        limit,
                        observed,
                    }
                )) if limit == maximum && observed == exact_peak
            ));
            assert_eq!(
                runtime.verification_storage_bytes(),
                verification_base,
                "failed verifier bounds admission restores the entry scope"
            );
        }
    }

    let retained_total = embedding_bytes + arrangement_bytes + verification_base;
    let mut deadline_observer = DeadlineAfter {
        continued_checkpoints: 0,
    };
    let mut deadline_runtime = Runtime::new(
        &mut deadline_observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: retained_total,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    deadline_runtime
        .set_embedding_exact_storage(embedding_bytes)
        .expect("deadline embedding fits");
    deadline_runtime
        .set_arrangement_exact_storage(arrangement_bytes)
        .expect("deadline arrangement fits");
    deadline_runtime
        .add_verification_storage(verification_base)
        .expect("deadline verification base fits");
    assert!(matches!(
        build_verifier_face_bounds(&faces, &mut deadline_runtime),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. }
        ))
    ));
    assert_eq!(
        deadline_runtime.verification_storage_bytes(),
        verification_base
    );

    let mut cancel_observer = CancelAfter {
        continued_checkpoints: 1,
    };
    let mut cancel_runtime = Runtime::new(
        &mut cancel_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    cancel_runtime
        .add_verification_storage(verification_base)
        .expect("cancel verification base fits");
    assert!(matches!(
        build_verifier_face_bounds(&faces, &mut cancel_runtime),
        Err(FacewiseAbort::Execution(
            GlobalFlatFoldabilityExecutionError::Cancelled
        ))
    ));
    assert_eq!(
        cancel_runtime.verification_storage_bytes(),
        verification_base
    );
}

#[test]
fn supporting_line_identity_is_exact_undirected_and_unbounded() {
    let huge: num_bigint::BigInt = num_bigint::BigInt::from(1_u8) << 4_096_usize;
    let line_face = |index, first, second| synthetic_face(index, vec![first, second], true);
    let faces = vec![
        line_face(0, integer_point(0, 0), integer_point(4, 4)),
        line_face(1, integer_point(8, 8), integer_point(2, 2)),
        line_face(2, integer_point(20, 20), integer_point(24, 24)),
        line_face(3, integer_point(0, 1), integer_point(4, 5)),
        line_face(4, integer_point(4, 4), integer_point(4, 8)),
        line_face(
            5,
            Point {
                x: Rational::from_integer(huge.clone()),
                y: Rational::from_integer(huge.clone()),
            },
            Point {
                x: Rational::from_integer(&huge + 4),
                y: Rational::from_integer(&huge + 4),
            },
        ),
        line_face(
            6,
            Point {
                x: Rational::from_integer(huge.clone()),
                y: Rational::from_integer(huge.clone()),
            },
            Point {
                x: Rational::from_integer(&huge + 4),
                y: Rational::from_integer(&huge + 5),
            },
        ),
    ];
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    assert!(supporting_lines_share_exact_line(&faces, (0, 0), (1, 0), &mut runtime).unwrap());
    assert!(supporting_lines_share_exact_line(&faces, (0, 0), (2, 0), &mut runtime).unwrap());
    assert!(supporting_lines_share_exact_line(&faces, (0, 0), (5, 0), &mut runtime).unwrap());
    assert!(!supporting_lines_share_exact_line(&faces, (0, 0), (3, 0), &mut runtime).unwrap());
    assert!(!supporting_lines_share_exact_line(&faces, (0, 0), (4, 0), &mut runtime).unwrap());
    assert!(!supporting_lines_share_exact_line(&faces, (0, 0), (6, 0), &mut runtime).unwrap());
    assert!(runtime.work.exact_operations > 0);
    assert!(runtime.work.exact_values > 0);
}

#[test]
fn supporting_line_deduplication_preserves_order_limits_and_control() {
    let mut canonical = synthetic_face(0, rectangle(0, 0, 4, 4), true);
    let mut duplicate = synthetic_face(1, rectangle(0, 0, 4, 4), false);
    duplicate.source.layer.face_key = canonical.source.layer.face_key;
    if duplicate.source.layer.face_id.canonical_bytes()
        < canonical.source.layer.face_id.canonical_bytes()
    {
        std::mem::swap(
            &mut canonical.source.layer.face_id,
            &mut duplicate.source.layer.face_id,
        );
    }
    let faces = vec![canonical, duplicate];
    let mut sorted_lines = faces
        .iter()
        .enumerate()
        .flat_map(|(face_index, face)| {
            (0..face.polygon.len()).map(move |edge_index| (face_index, edge_index))
        })
        .collect::<Vec<_>>();
    sorted_lines.sort_unstable_by_key(|(face_index, edge_index)| {
        (
            faces[*face_index].source.layer.face_key,
            *edge_index,
            faces[*face_index].source.layer.face_id.canonical_bytes(),
        )
    });

    let mut measuring_observer = NoopGlobalFlatFoldabilityObserver;
    let mut measuring_runtime = Runtime::new(
        &mut measuring_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let (measured_flags, measured_bytes) =
        supporting_line_keep_flags(&faces, &sorted_lines, &mut measuring_runtime)
            .expect("measure line deduplication");
    assert_eq!(measured_flags, [1, 0, 1, 0, 1, 0, 1, 0]);
    assert_eq!(measuring_runtime.exact_storage.arrangement_bytes, 0);
    let measured_operations = measuring_runtime.work.exact_operations;
    assert!(measured_operations > 0);

    for maximum in [measured_operations, measured_operations - 1] {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_exact_operations: maximum,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        let result = supporting_line_keep_flags(&faces, &sorted_lines, &mut runtime);
        if maximum == measured_operations {
            result.expect("line deduplication exact-work equality is admitted");
        } else {
            assert!(matches!(
                result,
                Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                        resource: FlatFoldabilityResource::ExactOperations,
                        limit,
                        observed,
                    }
                )) if limit == maximum && observed == measured_operations
            ));
        }
    }

    for maximum in [measured_bytes, measured_bytes - 1] {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: maximum,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        let result = supporting_line_keep_flags(&faces, &sorted_lines, &mut runtime);
        if maximum == measured_bytes {
            result.expect("line deduplication storage equality is admitted");
        } else {
            assert!(matches!(
                result,
                Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                        resource: FlatFoldabilityResource::CertificateBytes,
                        limit,
                        observed,
                    }
                )) if limit == maximum && observed == measured_bytes
            ));
        }
    }

    let mut deadline_observer = DeadlineAfter {
        continued_checkpoints: 0,
    };
    let mut deadline_runtime = Runtime::new(
        &mut deadline_observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: 0,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    assert!(matches!(
        supporting_line_keep_flags(&faces, &sorted_lines, &mut deadline_runtime),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. }
        ))
    ));

    let mut cancel_observer = CancelAfter {
        continued_checkpoints: 1,
    };
    let mut cancel_runtime = Runtime::new(
        &mut cancel_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    assert!(matches!(
        supporting_line_keep_flags(&faces, &sorted_lines, &mut cancel_runtime),
        Err(FacewiseAbort::Execution(
            GlobalFlatFoldabilityExecutionError::Cancelled
        ))
    ));
    assert_eq!(cancel_runtime.exact_storage.total(), Some(0));
}

#[test]
fn canonical_arrangement_is_invariant_to_storage_order_and_edge_direction() {
    let (paper, pattern, topology) = three_panel_accordion();
    let expected = geometry_arrangement_signature(&paper, &pattern, &topology);

    let mut reordered_pattern = pattern.clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    let reordered_topology = extract_faces_strict(FaceExtractionInput {
        identity_namespace: fixed_id::<ProjectId>(1),
        source_revision: topology.source_revision,
        paper: &paper,
        pattern: &reordered_pattern,
    })
    .expect("storage-reordered topology");
    assert_eq!(
        geometry_arrangement_signature(&paper, &reordered_pattern, &reordered_topology),
        expected
    );

    let mut reversed_edges = reordered_pattern;
    for edge in &mut reversed_edges.edges {
        std::mem::swap(&mut edge.start, &mut edge.end);
    }
    let reversed_topology = extract_faces_strict(FaceExtractionInput {
        identity_namespace: fixed_id::<ProjectId>(1),
        source_revision: topology.source_revision,
        paper: &paper,
        pattern: &reversed_edges,
    })
    .expect("direction-reversed topology");
    assert_eq!(
        geometry_arrangement_signature(&paper, &reversed_edges, &reversed_topology),
        expected
    );
}

#[test]
fn canonical_cells_cover_single_partial_three_ply_and_disjoint_regions() {
    let single_faces = vec![synthetic_face(0, rectangle(-2, -1, 3, 4), true)];
    let (single_pairs, single_cells) = build_test_arrangement(&single_faces);
    assert!(single_pairs.is_empty());
    assert_eq!(single_cells.len(), 1);
    assert_eq!(single_cells[0].covering_faces, [0]);
    let mut single_observer = NoopGlobalFlatFoldabilityObserver;
    let mut single_runtime = Runtime::new(
        &mut single_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    assert_eq!(
        signed_double_area(&single_cells[0].boundary, &mut single_runtime)
            .expect("single-cell area"),
        signed_double_area(&single_faces[0].polygon, &mut single_runtime)
            .expect("single-face area")
    );
    verify_canonical_overlap_cells(&single_faces, &single_cells, &mut single_runtime)
        .expect("single-face coverage reverifies");

    let faces = vec![
        synthetic_face(0, rectangle(0, 0, 6, 4), true),
        synthetic_face(1, rectangle(2, 0, 5, 4), false),
        synthetic_face(2, rectangle(3, 1, 4, 3), true),
        synthetic_face(3, rectangle(10, 0, 12, 2), false),
    ];
    let (pairs, cells) = build_test_arrangement(&faces);
    assert_eq!(
        pairs
            .iter()
            .map(|pair| (pair.first, pair.second))
            .collect::<Vec<_>>(),
        [(0, 1), (0, 2), (1, 2)]
    );
    assert!(
        cells
            .iter()
            .any(|cell| cell.covering_faces.as_slice() == [0])
    );
    assert!(
        cells
            .iter()
            .any(|cell| cell.covering_faces.as_slice() == [0, 1])
    );
    assert!(
        cells
            .iter()
            .any(|cell| cell.covering_faces.as_slice() == [0, 1, 2])
    );
    assert!(
        cells
            .iter()
            .any(|cell| cell.covering_faces.as_slice() == [3])
    );
    assert!(
        cells
            .iter()
            .all(|cell| !cell.covering_faces.contains(&3) || cell.covering_faces == [3])
    );
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    for cell in &cells {
        for index in 0..cell.boundary.len() {
            assert!(
                cross(
                    &cell.boundary[index],
                    &cell.boundary[(index + 1) % cell.boundary.len()],
                    &cell.boundary[(index + 2) % cell.boundary.len()],
                    &mut runtime,
                )
                .expect("strict-convex cross")
                .is_positive()
            );
        }
    }
    verify_canonical_overlap_cells(&faces, &cells, &mut runtime)
        .expect("mixed canonical arrangement reverifies");
}

#[test]
fn point_and_line_contacts_create_no_overlap_order() {
    let point_contact = vec![
        synthetic_face(0, rectangle(0, 0, 2, 2), true),
        synthetic_face(1, rectangle(2, 2, 4, 4), false),
    ];
    let (point_pairs, point_cells) = build_test_arrangement(&point_contact);
    assert!(point_pairs.is_empty());
    assert!(
        point_cells
            .iter()
            .all(|cell| cell.covering_faces.len() == 1)
    );

    let line_contact = vec![
        synthetic_face(0, rectangle(0, 0, 2, 2), true),
        synthetic_face(1, rectangle(0, -2, 2, 0), false),
    ];
    let (line_pairs, line_cells) = build_test_arrangement(&line_contact);
    assert!(line_pairs.is_empty());
    assert!(line_cells.iter().all(|cell| cell.covering_faces.len() == 1));
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    verify_canonical_overlap_cells(&point_contact, &point_cells, &mut runtime)
        .expect("point-contact cells reverify");
    verify_canonical_overlap_cells(&line_contact, &line_cells, &mut runtime)
        .expect("line-contact cells reverify");
}

#[test]
fn exact_bounds_prune_only_strict_separation_and_keep_boundary_contacts() {
    let first = rectangle(0, 0, 4, 4);
    let separated = rectangle(5, 0, 7, 2);
    let point_contact = rectangle(4, 4, 6, 6);
    let line_contact = rectangle(4, 1, 6, 3);
    let contained = rectangle(1, 1, 2, 2);
    let crossing = rectangle(3, -1, 5, 2);
    let huge: num_bigint::BigInt = num_bigint::BigInt::from(1_u8) << 4_096_usize;
    let huge_first = vec![
        Point {
            x: Rational::from_integer(huge.clone()),
            y: Rational::from_integer(0.into()),
        },
        Point {
            x: Rational::from_integer(&huge + 2),
            y: Rational::from_integer(0.into()),
        },
        Point {
            x: Rational::from_integer(&huge + 2),
            y: Rational::from_integer(2.into()),
        },
        Point {
            x: Rational::from_integer(huge.clone()),
            y: Rational::from_integer(2.into()),
        },
    ];
    let huge_second = vec![
        Point {
            x: Rational::from_integer(&huge + 3),
            y: Rational::from_integer(0.into()),
        },
        Point {
            x: Rational::from_integer(&huge + 5),
            y: Rational::from_integer(0.into()),
        },
        Point {
            x: Rational::from_integer(&huge + 5),
            y: Rational::from_integer(2.into()),
        },
        Point {
            x: Rational::from_integer(&huge + 3),
            y: Rational::from_integer(2.into()),
        },
    ];

    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let first_bounds = exact_axis_aligned_bounds(&first, &mut runtime).expect("first bounds");
    for (candidate, expected_separated) in [
        (&separated, true),
        (&point_contact, false),
        (&line_contact, false),
        (&contained, false),
        (&crossing, false),
    ] {
        let candidate_bounds =
            exact_axis_aligned_bounds(candidate, &mut runtime).expect("candidate bounds");
        assert_eq!(
            exact_axis_aligned_bounds_are_strictly_separated(
                &first,
                first_bounds,
                candidate,
                candidate_bounds,
                &mut runtime,
            )
            .expect("exact bounds comparison"),
            expected_separated
        );
    }
    let huge_first_bounds =
        exact_axis_aligned_bounds(&huge_first, &mut runtime).expect("first huge bounds");
    let huge_second_bounds =
        exact_axis_aligned_bounds(&huge_second, &mut runtime).expect("second huge bounds");
    assert!(
        exact_axis_aligned_bounds_are_strictly_separated(
            &huge_first,
            huge_first_bounds,
            &huge_second,
            huge_second_bounds,
            &mut runtime,
        )
        .expect("huge exact bounds comparison")
    );
}

#[test]
fn exact_bounds_disjointness_matches_unpruned_baseline() {
    let cases = [
        (
            vec![
                synthetic_cell(1, rectangle(0, 0, 2, 2), vec![0]),
                synthetic_cell(2, rectangle(3, 0, 5, 2), vec![1]),
            ],
            true,
        ),
        (
            vec![
                synthetic_cell(3, rectangle(0, 0, 2, 2), vec![0]),
                synthetic_cell(4, rectangle(2, 2, 4, 4), vec![1]),
            ],
            true,
        ),
        (
            vec![
                synthetic_cell(5, rectangle(0, 0, 2, 2), vec![0]),
                synthetic_cell(6, rectangle(2, 0, 4, 2), vec![1]),
            ],
            true,
        ),
        (
            vec![
                synthetic_cell(7, rectangle(0, 0, 4, 4), vec![0]),
                synthetic_cell(8, rectangle(1, 1, 2, 2), vec![1]),
            ],
            false,
        ),
        (
            vec![
                synthetic_cell(9, rectangle(0, 0, 4, 4), vec![0]),
                synthetic_cell(10, rectangle(3, -1, 5, 2), vec![1]),
            ],
            false,
        ),
    ];
    for (cells, expected_disjoint) in cases {
        let baseline = baseline_overlap_cell_interiors_are_disjoint(&cells);
        assert_eq!(baseline, expected_disjoint);
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        let optimized = verify_overlap_cell_interiors_are_disjoint(&cells, &mut runtime);
        assert_eq!(optimized.is_ok(), baseline);
        if !baseline {
            assert_certificate_reverification_failed(optimized);
        }
        assert_eq!(runtime.verification_storage_bytes(), 0);
    }
}

#[test]
fn convex_sat_matches_exact_clipping_for_overlap_containment_and_contacts() {
    let diamond = vec![
        integer_point(0, -3),
        integer_point(3, 0),
        integer_point(0, 3),
        integer_point(-3, 0),
    ];
    let cases = [
        (rectangle(0, 0, 2, 2), rectangle(3, 0, 5, 2)),
        (rectangle(0, 0, 2, 2), rectangle(2, 0, 4, 2)),
        (rectangle(0, 0, 2, 2), rectangle(2, 2, 4, 4)),
        (rectangle(0, 0, 5, 5), rectangle(1, 1, 2, 2)),
        (rectangle(0, 0, 4, 4), rectangle(3, -1, 5, 2)),
        (diamond, rectangle(-1, -1, 1, 1)),
    ];
    for (first, second) in cases {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        let intersection = convex_polygon_intersection(&first, &second, &mut runtime)
            .expect("exact clipped intersection");
        let expected = intersection.len() >= 3
            && signed_double_area(&intersection, &mut runtime)
                .expect("exact clipped area")
                .is_positive();
        assert_eq!(
            convex_polygon_interiors_overlap(&first, &second, &mut runtime)
                .expect("exact SAT predicate"),
            expected
        );
        assert_eq!(
            convex_polygon_interiors_overlap(&second, &first, &mut runtime)
                .expect("symmetric exact SAT predicate"),
            expected
        );
    }

    let container = rectangle(-5, -4, 6, 7);
    let contained = rectangle(-2, -1, 3, 4);
    let crossing = rectangle(4, 5, 8, 9);
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    assert!(
        convex_polygon_contains_polygon(&container, &contained, &mut runtime)
            .expect("contained polygon")
    );
    assert!(
        !convex_polygon_contains_polygon(&contained, &container, &mut runtime)
            .expect("reverse containment")
    );
    assert!(
        !convex_polygon_contains_polygon(&container, &crossing, &mut runtime)
            .expect("partial crossing is not containment")
    );
}

#[test]
fn coincident_polygon_class_requires_identical_cell_coverage_membership() {
    let polygon_classes = [0_usize, 0, 1, 1];
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    assert!(
        consistent_polygon_class_coverage(&[0, 1, 2, 3], &polygon_classes, 0, 0, &mut runtime)
            .expect("both coincident faces cover the cell")
    );
    assert!(
        !consistent_polygon_class_coverage(&[], &polygon_classes, 1, 2, &mut runtime)
            .expect("neither coincident face covers the cell")
    );
    assert_certificate_reverification_failed(
        consistent_polygon_class_coverage(&[0], &polygon_classes, 0, 0, &mut runtime).map(drop),
    );
}

#[test]
fn exact_bounds_disjointness_preserves_work_storage_and_control_boundaries() {
    let cells = vec![
        synthetic_cell(1, rectangle(0, 0, 2, 2), vec![0]),
        synthetic_cell(2, rectangle(3, 0, 5, 2), vec![1]),
    ];
    let mut capacity_probe = Vec::<ExactAxisAlignedBounds>::new();
    capacity_probe
        .try_reserve_exact(cells.len())
        .expect("exact bounds capacity probe");
    let bounds_bytes = capacity_probe.capacity() * std::mem::size_of::<ExactAxisAlignedBounds>();

    let mut measuring_observer = NoopGlobalFlatFoldabilityObserver;
    let mut measuring_runtime = Runtime::new(
        &mut measuring_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    verify_overlap_cell_interiors_are_disjoint(&cells, &mut measuring_runtime)
        .expect("measure exact bounds work");
    let exact_operations = measuring_runtime.work.exact_operations;
    assert!(exact_operations > 0);

    for maximum in [exact_operations, exact_operations - 1] {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_exact_operations: maximum,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        let result = verify_overlap_cell_interiors_are_disjoint(&cells, &mut runtime);
        if maximum == exact_operations {
            result.expect("exact work equality is admitted");
        } else {
            assert!(matches!(
                result,
                Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                        resource: FlatFoldabilityResource::ExactOperations,
                        limit,
                        observed,
                    }
                )) if limit == maximum && observed == exact_operations
            ));
        }
        assert_eq!(runtime.verification_storage_bytes(), 0);
    }

    for maximum in [bounds_bytes, bounds_bytes - 1] {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: maximum,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        let result = verify_overlap_cell_interiors_are_disjoint(&cells, &mut runtime);
        if maximum == bounds_bytes {
            result.expect("exact bounds storage equality is admitted");
        } else {
            assert!(matches!(
                result,
                Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                        resource: FlatFoldabilityResource::CertificateBytes,
                        limit,
                        observed,
                    }
                )) if limit == maximum && observed == bounds_bytes
            ));
        }
        assert_eq!(runtime.verification_storage_bytes(), 0);
    }

    let mut deadline_observer = DeadlineAfter {
        continued_checkpoints: 0,
    };
    let mut deadline_runtime = Runtime::new(
        &mut deadline_observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: 0,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    assert!(matches!(
        verify_overlap_cell_interiors_are_disjoint(&cells, &mut deadline_runtime),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. }
        ))
    ));
    assert_eq!(deadline_runtime.verification_storage_bytes(), 0);

    let mut cancel_observer = AlwaysCancel;
    let mut cancel_runtime = Runtime::new(
        &mut cancel_observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: 0,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    assert!(matches!(
        verify_overlap_cell_interiors_are_disjoint(&cells, &mut cancel_runtime),
        Err(FacewiseAbort::Execution(
            GlobalFlatFoldabilityExecutionError::Cancelled
        ))
    ));
    assert_eq!(cancel_runtime.verification_storage_bytes(), 0);

    let mut mid_cancel_observer = CancelAfter {
        continued_checkpoints: 1,
    };
    let mut mid_cancel_runtime = Runtime::new(
        &mut mid_cancel_observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: bounds_bytes,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    assert!(matches!(
        verify_overlap_cell_interiors_are_disjoint(&cells, &mut mid_cancel_runtime),
        Err(FacewiseAbort::Execution(
            GlobalFlatFoldabilityExecutionError::Cancelled
        ))
    ));
    assert_eq!(mid_cancel_runtime.verification_storage_bytes(), 0);
}

#[test]
fn canonical_reconstruction_preserves_limits_control_and_live_storage_accounting() {
    let faces = vec![
        synthetic_face(0, rectangle(0, 0, 4, 4), true),
        synthetic_face(1, rectangle(1, 0, 3, 4), false),
    ];
    let (_, cells) = build_test_arrangement(&faces);
    assert!(cells.len() > 1);
    let retained_bytes = arrangement_boundary_bytes(&cells);

    let mut limited_observer = NoopGlobalFlatFoldabilityObserver;
    let mut limited = Runtime::new(
        &mut limited_observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: retained_bytes,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    limited
        .set_arrangement_exact_storage(retained_bytes)
        .expect("retained source arrangement fits at equality");
    limited.work.arrangement_segments = 17;
    limited.work.overlap_cells = cells.len();
    let saved_storage = limited.exact_storage;
    assert!(matches!(
        verify_canonical_overlap_cells(&faces, &cells, &mut limited),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit,
                observed,
            }
        )) if limit == retained_bytes && observed > limit
    ));
    assert_eq!(limited.exact_storage.total(), saved_storage.total());
    assert_eq!(limited.work.arrangement_segments, 17);
    assert_eq!(limited.work.overlap_cells, cells.len());

    let mut cell_limit_observer = NoopGlobalFlatFoldabilityObserver;
    let mut cell_limited = Runtime::new(
        &mut cell_limit_observer,
        GlobalFlatFoldabilityLimits {
            max_overlap_cells: cells.len() - 1,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    assert!(matches!(
        verify_canonical_overlap_cells(&faces, &cells, &mut cell_limited),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::OverlapArrangementLimitReached {
                resource: FlatFoldabilityResource::OverlapCells,
                limit,
                observed,
            }
        )) if limit == cells.len() - 1 && observed > limit
    ));

    let mut deadline_observer = DeadlineAfter {
        continued_checkpoints: 2,
    };
    let mut deadline_runtime = Runtime::new(
        &mut deadline_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    deadline_runtime
        .set_arrangement_exact_storage(retained_bytes)
        .expect("deadline fixture retains the source arrangement");
    let deadline_storage = deadline_runtime.exact_storage;
    assert!(matches!(
        verify_canonical_overlap_cells(&faces, &cells, &mut deadline_runtime),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. }
        ))
    ));
    assert_eq!(
        deadline_runtime.exact_storage.total(),
        deadline_storage.total()
    );

    let mut cancel_observer = CancelAfter {
        continued_checkpoints: 2,
    };
    let mut cancel_runtime = Runtime::new(
        &mut cancel_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    cancel_runtime
        .set_arrangement_exact_storage(retained_bytes)
        .expect("cancel fixture retains the source arrangement");
    let cancel_storage = cancel_runtime.exact_storage;
    assert!(matches!(
        verify_canonical_overlap_cells(&faces, &cells, &mut cancel_runtime),
        Err(FacewiseAbort::Execution(
            GlobalFlatFoldabilityExecutionError::Cancelled
        ))
    ));
    assert_eq!(cancel_runtime.exact_storage.total(), cancel_storage.total());
}

#[test]
fn three_panel_geometry_builds_and_reverifies_a_facewise_certificate() {
    let (paper, pattern, topology) = three_panel_accordion();
    let canonical_faces = topology
        .faces
        .iter()
        .map(|face| LayerFace {
            face_id: face.id,
            face_key: face.key,
        })
        .collect::<Vec<_>>();
    let provenance = GlobalFlatFoldabilityProvenance {
        identity_namespace: Some(fixed_id(1)),
        source_revision: topology.source_revision,
        source_fingerprint: Some(crate::fold_model_fingerprint_v1(&pattern, &paper)),
        model_id: crate::GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
    };
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    runtime
        .advance(
            GlobalFlatFoldabilityPhase::BuildingFlatEmbedding,
            Some(canonical_faces.len()),
        )
        .expect("embedding phase");
    let embedding =
        build_flat_embedding(&paper, &pattern, &topology, &canonical_faces, &mut runtime)
            .expect("exact accordion embedding");
    assert_eq!(embedding.faces.len(), 3);
    runtime
        .advance(GlobalFlatFoldabilityPhase::BuildingOverlapArrangement, None)
        .expect("arrangement phase");
    let pairs = build_overlap_pairs(&embedding.faces, &mut runtime).expect("overlap pairs");
    let cells = build_overlap_cells(&embedding.faces, &pairs, &mut runtime).expect("atomic cells");
    let expected_pair_count = pairs.len();
    assert_eq!(pairs.len(), 3);
    assert_eq!(runtime.work.overlap_face_pairs, expected_pair_count);
    assert!(cells.iter().any(|cell| cell.covering_faces.len() == 3));
    assert_eq!(cells.len(), 1);
    let original_cell = &cells[0];
    let min_x = original_cell
        .boundary
        .iter()
        .map(|point| point.x.clone())
        .min()
        .expect("cell minimum x");
    let max_x = original_cell
        .boundary
        .iter()
        .map(|point| point.x.clone())
        .max()
        .expect("cell maximum x");
    let min_y = original_cell
        .boundary
        .iter()
        .map(|point| point.y.clone())
        .min()
        .expect("cell minimum y");
    let max_y = original_cell
        .boundary
        .iter()
        .map(|point| point.y.clone())
        .max()
        .expect("cell maximum y");
    let split_x = (&min_x + &max_x) / Rational::from_integer(2.into());
    let split_first = Point {
        x: split_x.clone(),
        y: min_y,
    };
    let split_second = Point {
        x: split_x,
        y: max_y,
    };
    let first_boundary = clip_polygon_halfplane(
        &original_cell.boundary,
        &split_first,
        &split_second,
        true,
        0,
        &mut runtime,
    )
    .expect("left atomic half");
    let second_boundary = clip_polygon_halfplane(
        &original_cell.boundary,
        &split_first,
        &split_second,
        false,
        exact_storage_bytes_points(&first_boundary).expect("first half bytes"),
        &mut runtime,
    )
    .expect("right atomic half");
    let artificially_split_cells = [first_boundary, second_boundary]
        .into_iter()
        .map(|boundary| {
            let key = overlap_cell_key(
                &boundary,
                &original_cell.covering_faces,
                &embedding.faces,
                &mut runtime,
            )
            .expect("derived split-cell key");
            OverlapCell {
                key,
                boundary,
                covering_faces: original_cell.covering_faces.clone(),
            }
        })
        .collect::<Vec<_>>();
    let mut split_observer = NoopGlobalFlatFoldabilityObserver;
    let mut split_runtime = Runtime::new(
        &mut split_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    split_runtime.phase = GlobalFlatFoldabilityPhase::BuildingOverlapArrangement;
    split_runtime
        .set_embedding_exact_storage(
            exact_storage_bytes_embedding(&embedding).expect("embedding exact bytes"),
        )
        .expect("embedding storage fits");
    split_runtime
        .set_overlap_pairs(pairs.len())
        .expect("overlap pairs fit");
    split_runtime
        .set_overlap_cells(artificially_split_cells.len())
        .expect("two split cells fit");
    split_runtime
        .set_arrangement_exact_storage(
            artificially_split_cells
                .iter()
                .map(|cell| {
                    exact_storage_bytes_points(&cell.boundary).expect("cell boundary bytes")
                })
                .fold(0_usize, usize::saturating_add),
        )
        .expect("split-cell exact storage fits");
    assert!(matches!(
        solve_layer_order(
            embedding.clone(),
            pairs.clone(),
            artificially_split_cells,
            provenance,
            None,
            &mut split_runtime,
        ),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed
            }
        ))
    ));

    let success = solve_layer_order(
        embedding.clone(),
        pairs,
        cells.clone(),
        provenance,
        None,
        &mut runtime,
    )
    .expect("accordion has a verified layer order");
    assert_eq!(success.layer_order.material_faces.len(), 3);
    assert_eq!(success.layer_order.overlap_cells.len(), cells.len());
    assert_eq!(success.layer_order.face_pair_orders.len(), 3);
    let proof_summary = success
        .layer_order
        .proof_summary
        .expect("verified proof summary");
    assert_eq!(proof_summary.overlap_face_pairs, expected_pair_count);
    assert_eq!(proof_summary.constraints, runtime.work.constraints);

    let face_indexes = embedding
        .faces
        .iter()
        .enumerate()
        .map(|(index, face)| (face.source.layer.face_id, index))
        .collect::<HashMap<_, _>>();
    let mut pair_values = PairValues::default();
    for order in &success.layer_order.face_pair_orders {
        let lower = face_indexes[&order.lower_face.face_id];
        let upper = face_indexes[&order.upper_face.face_id];
        let pair = ordered_pair(lower, upper);
        pair_values.insert(pair, upper == pair.1);
    }
    verify_layer_order_snapshot(
        &success.layer_order,
        &embedding,
        &cells,
        &pair_values,
        provenance,
        &mut runtime,
    )
    .expect("untampered snapshot reverifies");
    let required = RequiredLayerOrderPair {
        lower_face: success.layer_order.face_pair_orders[0].lower_face,
        upper_face: success.layer_order.face_pair_orders[0].upper_face,
    };
    verify_required_pair_orders_against_snapshot(&success.layer_order, &[required], &mut runtime)
        .expect("raw required faces and direction rejoin the completed snapshot");
    let reversed = RequiredLayerOrderPair {
        lower_face: required.upper_face,
        upper_face: required.lower_face,
    };
    assert!(matches!(
        verify_required_pair_orders_against_snapshot(
            &success.layer_order,
            &[reversed],
            &mut runtime,
        ),
        Err(FacewiseAbort::RequiredLayerOrder(
            RequiredLayerOrderError::CertificateReverificationFailed
        ))
    ));
    let mut noncanonical_embedding = embedding.clone();
    noncanonical_embedding.faces.swap(0, 1);
    assert!(!required_face_registry_is_strictly_canonical(
        &noncanonical_embedding
    ));

    let untampered = success.layer_order;
    let actual_certificate_bytes = untampered
        .proof_summary
        .expect("certificate summary")
        .certificate_bytes;
    let mut exact_limit_observer = NoopGlobalFlatFoldabilityObserver;
    let mut exact_limit_runtime = Runtime::new(
        &mut exact_limit_observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: actual_certificate_bytes,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    assert_eq!(
        serialized_certificate_size(&untampered, &mut exact_limit_runtime)
            .expect("real certificate fits its exact serialized size"),
        actual_certificate_bytes
    );
    let mut one_byte_short_observer = NoopGlobalFlatFoldabilityObserver;
    let mut one_byte_short_runtime = Runtime::new(
        &mut one_byte_short_observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: actual_certificate_bytes - 1,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    assert!(matches!(
        serialized_certificate_size(&untampered, &mut one_byte_short_runtime),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit,
                observed,
            }
        )) if limit == actual_certificate_bytes - 1
            && observed == actual_certificate_bytes
    ));
    let mut deadline_observer = DeadlineAfter {
        continued_checkpoints: 1,
    };
    let mut deadline_runtime = Runtime::new(
        &mut deadline_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    deadline_runtime.phase = GlobalFlatFoldabilityPhase::VerifyingCertificate;
    assert!(matches!(
        serialized_certificate_size(&untampered, &mut deadline_runtime),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                phase: GlobalFlatFoldabilityPhase::VerifyingCertificate,
            }
        ))
    ));

    let mut tampered = untampered.clone();
    let cell = tampered
        .overlap_cells
        .iter_mut()
        .find(|cell| cell.bottom_to_top_faces.len() >= 2)
        .expect("multi-layer cell");
    cell.bottom_to_top_faces.swap(0, 1);
    assert!(
        verify_layer_order_snapshot(
            &tampered,
            &embedding,
            &cells,
            &pair_values,
            provenance,
            &mut runtime,
        )
        .is_err()
    );

    let mut duplicate_cell = untampered.clone();
    duplicate_cell
        .overlap_cells
        .push(duplicate_cell.overlap_cells[0].clone());
    assert!(
        verify_layer_order_snapshot(
            &duplicate_cell,
            &embedding,
            &cells,
            &pair_values,
            provenance,
            &mut runtime,
        )
        .is_err()
    );

    let mut forged_internal_cells = cells.clone();
    let forged_key = OverlapCellKey([0xa5; 32]);
    let original_key = forged_internal_cells[0].key;
    forged_internal_cells[0].key = forged_key;
    let mut forged_internal_and_snapshot_key = untampered.clone();
    forged_internal_and_snapshot_key
        .overlap_cells
        .iter_mut()
        .find(|cell| cell.cell_key == original_key)
        .expect("corresponding overlap-cell snapshot")
        .cell_key = forged_key;
    assert!(
        verify_layer_order_snapshot(
            &forged_internal_and_snapshot_key,
            &embedding,
            &forged_internal_cells,
            &pair_values,
            provenance,
            &mut runtime,
        )
        .is_err()
    );

    assert!(untampered.face_pair_orders.len() >= 2);
    let mut duplicate_pair = untampered.clone();
    duplicate_pair.face_pair_orders[1] = duplicate_pair.face_pair_orders[0].clone();
    assert!(
        verify_layer_order_snapshot(
            &duplicate_pair,
            &embedding,
            &cells,
            &pair_values,
            provenance,
            &mut runtime,
        )
        .is_err()
    );

    let mut reordered_pairs = untampered.clone();
    reordered_pairs.face_pair_orders.swap(0, 1);
    assert!(
        verify_layer_order_snapshot(
            &reordered_pairs,
            &embedding,
            &cells,
            &pair_values,
            provenance,
            &mut runtime,
        )
        .is_err()
    );

    let mut tampered_derivation = untampered.clone();
    let LayerOrderDerivation::FacewiseCertificate {
        overlap_cell_count, ..
    } = &mut tampered_derivation.provenance.derivation
    else {
        panic!("three panels use a facewise certificate derivation");
    };
    *overlap_cell_count += 1;
    assert!(
        verify_layer_order_snapshot(
            &tampered_derivation,
            &embedding,
            &cells,
            &pair_values,
            provenance,
            &mut runtime,
        )
        .is_err()
    );

    let original_certificate_bytes = runtime.work.certificate_bytes;
    let forged_certificate_bytes = original_certificate_bytes + 1;
    let mut tampered_bytes = untampered.clone();
    tampered_bytes
        .proof_summary
        .as_mut()
        .expect("facewise proof summary")
        .certificate_bytes = forged_certificate_bytes;
    runtime.work.certificate_bytes = forged_certificate_bytes;
    assert!(
        verify_layer_order_snapshot(
            &tampered_bytes,
            &embedding,
            &cells,
            &pair_values,
            provenance,
            &mut runtime,
        )
        .is_err()
    );
}

#[test]
fn exact_storage_budget_admits_128_mib_and_rejects_one_more_byte() {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    runtime
        .set_embedding_exact_storage(crate::DEFAULT_MAX_CERTIFICATE_BYTES)
        .expect("the exact 128 MiB boundary is admitted");
    runtime
        .ensure_transient_exact_storage(0)
        .expect("zero additional bytes remain admitted");
    assert!(matches!(
        runtime.ensure_transient_exact_storage(1),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit: crate::DEFAULT_MAX_CERTIFICATE_BYTES,
                observed,
            }
        )) if observed == crate::DEFAULT_MAX_CERTIFICATE_BYTES + 1
    ));
}

#[test]
fn storage_arithmetic_overflow_fails_closed_even_with_usize_max_limit() {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: usize::MAX,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    assert!(matches!(
        runtime.set_embedding_exact_storage(usize::MAX),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit: usize::MAX,
                observed: usize::MAX,
            }
        ))
    ));
    runtime.exact_storage.certificate_structure_bytes = usize::MAX - 1;
    assert!(matches!(
        runtime.add_certificate_structure_storage(2),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit: usize::MAX,
                observed: usize::MAX,
            }
        ))
    ));
}

#[test]
fn certificate_structure_and_verifier_storage_share_one_live_memory_limit() {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: 128,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    runtime
        .add_certificate_structure_storage(80)
        .expect("retained certificate structure fits");
    runtime
        .add_verification_storage(48)
        .expect("live verifier reconstruction fits at equality");
    assert_eq!(runtime.exact_storage.total(), Some(128));
    assert!(matches!(
        runtime.add_verification_storage(1),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit: 128,
                observed: 129,
            }
        ))
    ));
    assert_eq!(
        runtime.verification_storage_bytes(),
        48,
        "a rejected allocation is not committed to the live budget"
    );
    runtime.restore_verification_storage(0);
    runtime
        .ensure_transient_exact_storage(48)
        .expect("released verifier storage becomes available to a scoped value");
}

#[test]
fn retained_constraint_problem_and_regenerated_verifier_share_one_live_limit() {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: 128,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    runtime
        .add_constraint_storage(80)
        .expect("primary constraint problem fits");
    runtime
        .add_verification_storage(48)
        .expect("regenerated verifier fits at the shared equality boundary");
    assert_eq!(runtime.exact_storage.total(), Some(128));
    assert!(matches!(
        runtime.add_verification_storage(1),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit: 128,
                observed: 129,
            }
        ))
    ));
    runtime.restore_verification_storage(0);
    runtime
        .ensure_constraint_transient_storage(48)
        .expect("verification scope release restores the primary problem headroom");
    runtime.clear_constraint_storage();
    assert_eq!(runtime.exact_storage.total(), Some(0));
}

#[test]
fn retained_solver_assignment_remains_charged_until_pair_values_replace_it() {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: 128,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    runtime
        .add_constraint_storage(80)
        .expect("retained constraint problem fits");
    runtime
        .add_constraint_storage(48)
        .expect("returned assignment fits at equality");
    assert_eq!(runtime.exact_storage.total(), Some(128));
    assert!(matches!(
        runtime.add_certificate_structure_storage(1),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit: 128,
                observed: 129,
            }
        ))
    ));
    runtime.clear_constraint_storage();
    runtime
        .add_certificate_structure_storage(128)
        .expect("assignment and problem storage are released together after both values drop");
}

#[test]
fn released_required_overlay_is_not_charged_against_pair_value_replacement() {
    let required_assignments = vec![(0_usize, true)];
    let fixed_assignments = vec![Some(true)];
    let overlay_bytes = required_assignments.len() * std::mem::size_of::<(usize, bool)>()
        + fixed_assignments.len() * std::mem::size_of::<Option<bool>>();
    let pair_value_bytes = std::mem::size_of::<((usize, usize), bool)>();
    assert!(
        pair_value_bytes > overlay_bytes,
        "the replacement must define the live peak for this boundary fixture"
    );
    let retained_problem_and_assignment_bytes = 40;
    let exact_limit = retained_problem_and_assignment_bytes + pair_value_bytes;

    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: exact_limit,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    runtime
        .add_constraint_storage(retained_problem_and_assignment_bytes)
        .expect("retained problem and assignment fit");
    runtime
        .add_constraint_storage(overlay_bytes)
        .expect("required overlay fits before solver verification");
    let overlay = RequiredPairOverlay {
        fixed_assignments,
        required_assignments,
        storage_bytes: overlay_bytes,
    };
    assert_eq!(
        runtime.exact_storage.total(),
        Some(retained_problem_and_assignment_bytes + overlay_bytes)
    );
    assert!(matches!(
        runtime.add_certificate_structure_storage(pair_value_bytes),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit,
                observed,
            }
        )) if limit == exact_limit
            && observed
                == retained_problem_and_assignment_bytes
                    + overlay_bytes
                    + pair_value_bytes
    ));

    overlay
        .release(&mut runtime)
        .expect("dropped overlay releases only its own retained bytes");
    assert_eq!(
        runtime.exact_storage.total(),
        Some(retained_problem_and_assignment_bytes)
    );
    runtime
        .add_certificate_structure_storage(pair_value_bytes)
        .expect("the exact live replacement peak is admitted after overlay release");
    assert_eq!(runtime.exact_storage.total(), Some(exact_limit));

    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut one_byte_short = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: exact_limit - 1,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    one_byte_short
        .add_constraint_storage(retained_problem_and_assignment_bytes)
        .expect("retained problem and assignment fit");
    one_byte_short
        .add_constraint_storage(overlay_bytes)
        .expect("overlay is below the replacement peak");
    RequiredPairOverlay {
        fixed_assignments: vec![Some(true)],
        required_assignments: vec![(0, true)],
        storage_bytes: overlay_bytes,
    }
    .release(&mut one_byte_short)
    .expect("overlay release preserves the retained problem and assignment");
    assert!(matches!(
        one_byte_short.add_certificate_structure_storage(pair_value_bytes),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit,
                observed,
            }
        )) if limit == exact_limit - 1 && observed == exact_limit
    ));
    assert_eq!(
        one_byte_short.exact_storage.total(),
        Some(retained_problem_and_assignment_bytes)
    );

    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut inconsistent = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    inconsistent
        .add_constraint_storage(1)
        .expect("one retained byte");
    assert!(matches!(
        inconsistent.release_constraint_storage(2),
        Err(FacewiseAbort::Execution(
            GlobalFlatFoldabilityExecutionError::Internal {
                reason: GlobalFlatFoldabilityInternalError::ValidatedTopologyInvariantLost,
            }
        ))
    ));
    assert_eq!(
        inconsistent.exact_storage.constraint_bytes, 1,
        "an impossible release leaves the live accounting unchanged"
    );
}

#[test]
fn tuple_constraint_storage_accepts_exact_limit_and_rejects_one_byte_less_before_outer_alloc() {
    fn fixture() -> TupleConstraint {
        TupleConstraint {
            kind: FacewiseConstraintKind::Transitivity,
            variables: vec![0, 1, 2],
            allowed_rows: vec![0, 1, 2, 4, 5, 6],
            faces: vec![0, 1, 2],
            supporting_cell: None,
        }
    }
    let sizing = fixture();
    let nested = sizing.variables.capacity() * std::mem::size_of::<usize>()
        + sizing.allowed_rows.capacity() * std::mem::size_of::<u8>()
        + sizing.faces.capacity() * std::mem::size_of::<usize>();
    let exact_limit = nested + 4 * std::mem::size_of::<TupleConstraint>();

    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_constraints: 5_000_000,
            max_certificate_bytes: exact_limit,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    let mut constraints = Vec::new();
    push_constraint(
        &mut constraints,
        sizing,
        &mut runtime,
        ConstraintStorageScope::Primary,
        0,
    )
    .expect("exact tuple and outer Vec storage boundary is admitted");
    assert_eq!(runtime.exact_storage.total(), Some(exact_limit));

    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut over_limit = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_constraints: 5_000_000,
            max_certificate_bytes: exact_limit - 1,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    let mut rejected = Vec::new();
    assert!(matches!(
        push_constraint(
            &mut rejected,
            fixture(),
            &mut over_limit,
            ConstraintStorageScope::Primary,
            0,
        ),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit,
                observed,
            }
        )) if limit == exact_limit - 1 && observed == exact_limit
    ));
    assert_eq!(
        rejected.capacity(),
        0,
        "the large outer Vec is not allocated"
    );
    assert_eq!(over_limit.exact_storage.constraint_bytes, 0);
}

#[test]
fn compact_family_storage_accepts_exact_limit_and_rejects_one_byte_less() {
    fn fixture() -> TransitivityConstraintFamily {
        TransitivityConstraintFamily {
            covering_faces: vec![0, 1, 2],
            pair_variables: vec![0, 1, 2],
            supporting_cell: OverlapCellKey([3; 32]),
        }
    }

    let sizing = fixture();
    let exact_limit = sizing.covering_faces.capacity() * std::mem::size_of::<usize>()
        + sizing.pair_variables.capacity() * std::mem::size_of::<usize>()
        + std::mem::size_of::<TransitivityConstraintFamily>();
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: exact_limit,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    let mut families = Vec::new();
    push_transitivity_constraint_family(
        &mut families,
        sizing,
        1,
        &mut runtime,
        ConstraintStorageScope::Primary,
    )
    .expect("the exact compact-family retained boundary is admitted");
    assert_eq!(runtime.exact_storage.constraint_bytes, exact_limit);

    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut rejected_runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: exact_limit - 1,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    let mut rejected = Vec::new();
    assert!(matches!(
        push_transitivity_constraint_family(
            &mut rejected,
            fixture(),
            1,
            &mut rejected_runtime,
            ConstraintStorageScope::Primary,
        ),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit,
                observed,
            }
        )) if limit == exact_limit - 1 && observed == exact_limit
    ));
    assert_eq!(rejected.capacity(), 0);
    assert_eq!(rejected_runtime.exact_storage.constraint_bytes, 0);
}

#[test]
fn compact_logical_constraint_batches_preserve_limit_and_checkpoint_boundaries() {
    let mut observer = CountingObserver::default();
    {
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_constraints: 2_049,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        admit_constraint_batch(0, 2_049, &mut runtime)
            .expect("the exact logical constraint limit is admitted");
    }
    assert_eq!(
        observer.checkpoints, 4,
        "preflight plus three 1024-record batches"
    );

    let mut observer = CountingObserver::default();
    let result = {
        let mut one_short = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_constraints: 2_048,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        admit_constraint_batch(0, 2_049, &mut one_short)
    };
    assert!(matches!(
        result,
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ConstraintLimitReached {
                limit: 2_048,
                observed: 2_049,
            }
        ))
    ));
    assert_eq!(observer.checkpoints, 4);
}

#[test]
fn tuple_constraint_outer_growth_accounts_old_and_new_buffers_at_the_reallocation_peak() {
    fn fixture() -> TupleConstraint {
        TupleConstraint {
            kind: FacewiseConstraintKind::Antisymmetry,
            variables: vec![0],
            allowed_rows: vec![0, 1],
            faces: vec![0, 1],
            supporting_cell: None,
        }
    }

    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let mut constraints = Vec::new();
    for _ in 0..4 {
        push_constraint(
            &mut constraints,
            fixture(),
            &mut runtime,
            ConstraintStorageScope::Primary,
            0,
        )
        .expect("initial four-entry buffer");
    }
    assert_eq!(constraints.capacity(), 4);
    let retained_before = runtime.exact_storage.constraint_bytes;
    let next = fixture();
    let nested = next.variables.capacity() * std::mem::size_of::<usize>()
        + next.allowed_rows.capacity() * std::mem::size_of::<u8>()
        + next.faces.capacity() * std::mem::size_of::<usize>();
    let outer_delta = 4 * std::mem::size_of::<TupleConstraint>();
    let old_outer = 4 * std::mem::size_of::<TupleConstraint>();
    let peak = retained_before + nested + outer_delta + old_outer;
    runtime.limits.max_certificate_bytes = peak - 1;
    assert!(matches!(
        push_constraint(
            &mut constraints,
            next,
            &mut runtime,
            ConstraintStorageScope::Primary,
            0,
        ),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit,
                observed,
            }
        )) if limit == peak - 1 && observed == peak
    ));
    assert_eq!(constraints.len(), 4);
    assert_eq!(constraints.capacity(), 4);
    assert_eq!(runtime.exact_storage.constraint_bytes, retained_before);

    runtime.limits.max_certificate_bytes = peak;
    push_constraint(
        &mut constraints,
        fixture(),
        &mut runtime,
        ConstraintStorageScope::Primary,
        0,
    )
    .expect("the exact old-plus-new reallocation peak is admitted");
    assert_eq!(constraints.len(), 5);
}

#[test]
fn constraint_storage_overflow_is_fail_closed() {
    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: usize::MAX,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    runtime.exact_storage.constraint_bytes = usize::MAX - 1;
    assert!(matches!(
        runtime.add_constraint_storage(2),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit: usize::MAX,
                observed: usize::MAX,
            }
        ))
    ));
}

#[test]
fn deadline_and_cancel_are_observed_before_constraint_outer_allocation() {
    fn fixture() -> TupleConstraint {
        TupleConstraint {
            kind: FacewiseConstraintKind::Antisymmetry,
            variables: vec![0],
            allowed_rows: vec![0, 1],
            faces: vec![0, 1],
            supporting_cell: None,
        }
    }

    let mut deadline_observer = DeadlineAfter {
        continued_checkpoints: 0,
    };
    let mut deadline_runtime = Runtime::new(
        &mut deadline_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let mut constraints = Vec::new();
    assert!(matches!(
        push_constraint(
            &mut constraints,
            fixture(),
            &mut deadline_runtime,
            ConstraintStorageScope::Primary,
            0,
        ),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. }
        ))
    ));
    assert_eq!(constraints.capacity(), 0);
    assert_eq!(deadline_runtime.exact_storage.constraint_bytes, 0);

    let mut cancel_observer = AlwaysCancel;
    let mut cancel_runtime = Runtime::new(
        &mut cancel_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let mut constraints = Vec::new();
    assert!(matches!(
        push_constraint(
            &mut constraints,
            fixture(),
            &mut cancel_runtime,
            ConstraintStorageScope::Primary,
            0,
        ),
        Err(FacewiseAbort::Execution(
            GlobalFlatFoldabilityExecutionError::Cancelled
        ))
    ));
    assert_eq!(constraints.capacity(), 0);
    assert_eq!(cancel_runtime.exact_storage.constraint_bytes, 0);
}

#[test]
fn arrangement_cell_boundary_and_snapshot_exact_storage_are_aggregated() {
    let embedding_points = vec![integer_point(-2, -2), integer_point(2, -2)];
    let cell_boundary = vec![
        integer_point(-1, -1),
        integer_point(1, -1),
        integer_point(1, 1),
        integer_point(-1, 1),
    ];
    let embedding_bytes = exact_storage_bytes_points(&embedding_points).expect("embedding bytes");
    let boundary_bytes = exact_storage_bytes_points(&cell_boundary).expect("cell boundary bytes");
    let snapshot_bytes = boundary_bytes;
    let exact_limit = embedding_bytes + boundary_bytes + snapshot_bytes;

    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: exact_limit,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    runtime
        .set_embedding_exact_storage(embedding_bytes)
        .expect("embedding storage fits");
    runtime
        .set_arrangement_exact_storage(boundary_bytes)
        .expect("cell boundary storage fits");
    runtime
        .add_snapshot_exact_storage(snapshot_bytes)
        .expect("aggregate equality is admitted");
    assert_eq!(runtime.exact_storage.total(), Some(exact_limit));

    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut over_limit = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: exact_limit - 1,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    over_limit
        .set_embedding_exact_storage(embedding_bytes)
        .expect("embedding storage fits below the aggregate limit");
    over_limit
        .set_arrangement_exact_storage(boundary_bytes)
        .expect("cell boundary storage fits below the aggregate limit");
    assert!(matches!(
        over_limit.add_snapshot_exact_storage(snapshot_bytes),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit,
                observed,
            }
        )) if limit == exact_limit - 1 && observed == exact_limit
    ));
}

#[test]
fn embedding_storage_stops_before_source_polygon_clones_exceed_the_limit() {
    let (paper, pattern, topology) = three_panel_accordion();
    let canonical_faces = topology
        .faces
        .iter()
        .map(|face| LayerFace {
            face_id: face.id,
            face_key: face.key,
        })
        .collect::<Vec<_>>();
    let mut sizing_observer = NoopGlobalFlatFoldabilityObserver;
    let mut sizing_runtime = Runtime::new(
        &mut sizing_observer,
        GlobalFlatFoldabilityLimits::default(),
        zero_work(),
    );
    let source_points = pattern
        .vertices
        .iter()
        .map(|vertex| {
            point_from_binary64(vertex.position.x, vertex.position.y, &mut sizing_runtime)
                .expect("fixture coordinate")
        })
        .collect::<Vec<_>>();
    let source_vertex_bytes =
        exact_storage_bytes_points(&source_points).expect("source exact bytes");
    let first_clone_bytes =
        exact_storage_bytes_point(&source_points[0]).expect("first clone bytes");

    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: source_vertex_bytes,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    let result = build_flat_embedding(&paper, &pattern, &topology, &canonical_faces, &mut runtime);
    assert!(matches!(
        result,
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit,
                observed,
            }
        )) if limit == source_vertex_bytes
            && observed == source_vertex_bytes + first_clone_bytes
    ));
    assert_eq!(
        runtime.exact_storage.embedding_bytes, source_vertex_bytes,
        "the rejected clone is never committed to retained storage"
    );
}

#[test]
fn cell_key_encoding_admits_one_boundary_copy_and_rejects_one_byte_less() {
    let boundary = vec![
        integer_point(-1, -1),
        integer_point(1, -1),
        integer_point(1, 1),
        integer_point(-1, 1),
    ];
    let boundary_bytes = exact_storage_bytes_points(&boundary).expect("cell boundary exact bytes");
    let canonical_structure_bytes =
        boundary.len() * std::mem::size_of::<Vec<u8>>() + std::mem::size_of::<Vec<Vec<u8>>>();
    let exact_limit = boundary_bytes * 2 + canonical_structure_bytes;
    let faces = vec![synthetic_face(0, boundary.clone(), true)];

    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut runtime = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: exact_limit,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    runtime
        .set_arrangement_exact_storage(boundary_bytes)
        .expect("retained boundary fits");
    overlap_cell_key(&boundary, &[0], &faces, &mut runtime)
        .expect("one transient canonical boundary copy fits at equality");

    let mut observer = NoopGlobalFlatFoldabilityObserver;
    let mut over_limit = Runtime::new(
        &mut observer,
        GlobalFlatFoldabilityLimits {
            max_certificate_bytes: exact_limit - 1,
            ..GlobalFlatFoldabilityLimits::default()
        },
        zero_work(),
    );
    over_limit
        .set_arrangement_exact_storage(boundary_bytes)
        .expect("retained boundary fits below the aggregate limit");
    assert!(matches!(
        overlap_cell_key(&boundary, &[0], &faces, &mut over_limit),
        Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                resource: FlatFoldabilityResource::CertificateBytes,
                limit,
                observed,
            }
        )) if limit == exact_limit - 1 && observed == exact_limit
    ));
}
