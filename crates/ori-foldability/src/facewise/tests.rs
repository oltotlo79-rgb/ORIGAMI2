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
            .iter()
            .filter(|constraint| constraint.kind == FacewiseConstraintKind::TacoTortilla)
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
            .iter()
            .filter(|constraint| constraint.kind == FacewiseConstraintKind::TacoTaco)
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
            .iter()
            .filter(|constraint| constraint.kind == FacewiseConstraintKind::TacoTaco)
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
