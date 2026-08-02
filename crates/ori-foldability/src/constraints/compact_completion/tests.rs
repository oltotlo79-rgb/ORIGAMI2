use super::*;

fn empty_constraints() -> ConstraintSet {
    ConstraintSet::from_explicit(Vec::new())
}

fn compact_family(
    faces: &[usize],
    pair_variables: &[usize],
    cell: u64,
) -> TransitivityConstraintFamily {
    let mut cell_key = [0_u8; 32];
    cell_key[..8].copy_from_slice(&cell.to_be_bytes());
    TransitivityConstraintFamily {
        covering_faces: faces.to_vec(),
        pair_variables: pair_variables.to_vec(),
        supporting_cell: OverlapCellKey(cell_key),
    }
}

fn compact_order_scratch(maximum_ply: usize) -> CompactOrderScratch {
    let mut indegrees = Vec::new();
    let mut ranks = Vec::new();
    let mut selected = Vec::new();
    indegrees.reserve_exact(maximum_ply);
    ranks.reserve_exact(maximum_ply);
    selected.reserve_exact(maximum_ply);
    CompactOrderScratch {
        indegrees,
        ranks,
        selected,
    }
}

fn independent_solver_memory_bytes(variable_count: usize) -> usize {
    [
        (variable_count, std::mem::size_of::<u8>()),
        (variable_count, std::mem::size_of::<u8>()),
        (variable_count, std::mem::size_of::<usize>()),
        (variable_count, std::mem::size_of::<u8>()),
        (variable_count, std::mem::size_of::<(usize, usize)>()),
        (variable_count, std::mem::size_of::<Range<usize>>()),
        (variable_count, std::mem::size_of::<usize>()),
        (
            variable_count,
            std::mem::size_of::<SearchFrame>().max(std::mem::size_of::<CompactSearchFrame>()),
        ),
        (variable_count, std::mem::size_of::<(usize, u8)>()),
    ]
    .into_iter()
    .map(|(count, element_size)| {
        count
            .checked_mul(element_size)
            .expect("bounded independent allocation")
    })
    .try_fold(0_usize, |total, bytes| total.checked_add(bytes))
    .expect("bounded independent solver memory")
}

fn independent_compact_memory_bytes(
    family_plies: &[usize],
    variable_count: usize,
    explicit_len: usize,
    compact_explicit_len: usize,
    compact_explicit_incidence_len: usize,
) -> usize {
    let word_bits = usize::BITS as usize;
    let maximum_ply = family_plies.iter().copied().max().unwrap_or(0);
    let order_scratch = maximum_ply
        .checked_mul(2 * std::mem::size_of::<usize>() + std::mem::size_of::<u8>())
        .expect("bounded independent compact memory");
    let closure_incidence_count = family_plies
        .iter()
        .copied()
        .map(|ply| choose_two(ply).expect("bounded closure incidence count"))
        .sum::<usize>();
    let closure_word_count = family_plies
        .iter()
        .copied()
        .map(|ply| {
            let words_per_row = ply.div_ceil(word_bits);
            ply * words_per_row
        })
        .sum::<usize>();
    let closure_trail_records = family_plies
        .iter()
        .copied()
        .map(|ply| ply * ply)
        .sum::<usize>();
    let closure_scratch = if family_plies.is_empty() {
        0
    } else {
        variable_count * std::mem::size_of::<usize>()
            + (family_plies.len() + 1) * std::mem::size_of::<usize>()
            + (variable_count + 1) * std::mem::size_of::<usize>()
            + closure_incidence_count * std::mem::size_of::<CompactPairIncidence>()
            + closure_word_count * std::mem::size_of::<usize>()
            + variable_count * std::mem::size_of::<usize>()
            + closure_trail_records * std::mem::size_of::<CompactReachabilityWordChange>()
    };
    let explicit_scratch = if compact_explicit_len == 0 {
        0
    } else {
        variable_count * std::mem::size_of::<usize>()
            + (variable_count + 1) * std::mem::size_of::<usize>()
            + compact_explicit_incidence_len * std::mem::size_of::<usize>()
            + compact_explicit_len * std::mem::size_of::<usize>()
            + explicit_len * std::mem::size_of::<u8>()
    };
    order_scratch + closure_scratch + explicit_scratch
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompactClosureOracleOutcome {
    Stable,
    Conflict,
    Invalid,
}

fn reference_compact_closure(
    candidate: &mut [u8],
    constraints: &ConstraintSet,
) -> CompactClosureOracleOutcome {
    let mut trail = Vec::with_capacity(candidate.len());
    let maximum_ply = constraints.transitivity.maximum_ply();
    let mut reachability = Vec::with_capacity(
        compact_reachability_word_count(maximum_ply).expect("small reference closure"),
    );
    for _ in 0..=candidate.len() {
        match propagate_compact_transitivity_reference(
            candidate,
            constraints,
            0,
            &mut trail,
            &mut reachability,
            &mut |_, _| ConstraintSolverControl::Continue,
        ) {
            Ok(CompactTransitivityPropagationResult::Stable) => {
                return CompactClosureOracleOutcome::Stable;
            }
            Ok(CompactTransitivityPropagationResult::Changed) => {}
            Ok(CompactTransitivityPropagationResult::Conflict) => {
                return CompactClosureOracleOutcome::Conflict;
            }
            Ok(CompactTransitivityPropagationResult::InvalidConstraint) | Err(_) => {
                return CompactClosureOracleOutcome::Invalid;
            }
        }
    }
    CompactClosureOracleOutcome::Invalid
}

fn incremental_compact_closure(
    candidate: &mut [u8],
    constraints: &ConstraintSet,
) -> CompactClosureOracleOutcome {
    let mut trail = Vec::with_capacity(candidate.len());
    let mut worklist =
        match CompactExplicitWorklist::try_new(candidate.len(), constraints, &mut |_, _| {
            ConstraintSolverControl::Continue
        }) {
            Ok(worklist) => worklist,
            Err(_) => return CompactClosureOracleOutcome::Invalid,
        };
    let mut closure =
        match CompactTransitiveClosure::try_new(candidate.len(), constraints, &mut |_, _| {
            ConstraintSolverControl::Continue
        }) {
            Ok(closure) => closure,
            Err(_) => return CompactClosureOracleOutcome::Invalid,
        };
    if closure
        .enqueue_initial_domains(candidate, &mut |_, _| ConstraintSolverControl::Continue)
        .is_err()
    {
        return CompactClosureOracleOutcome::Invalid;
    }
    match closure.propagate(
        candidate,
        constraints,
        0,
        &mut trail,
        &mut worklist,
        &mut |_, _| ConstraintSolverControl::Continue,
    ) {
        CompactPropagationResult::Stable => CompactClosureOracleOutcome::Stable,
        CompactPropagationResult::Conflict => CompactClosureOracleOutcome::Conflict,
        _ => CompactClosureOracleOutcome::Invalid,
    }
}

fn assert_compact_closure_matches_reference(
    domains: &[u8],
    constraints: &ConstraintSet,
    label: &str,
) {
    let mut expected = domains.to_vec();
    let mut actual = domains.to_vec();
    let expected_outcome = reference_compact_closure(&mut expected, constraints);
    let actual_outcome = incremental_compact_closure(&mut actual, constraints);
    assert_eq!(actual_outcome, expected_outcome, "{label}");
    if actual_outcome == CompactClosureOracleOutcome::Stable {
        assert_eq!(actual, expected, "{label}");
    }
}

#[test]
fn incremental_compact_closure_matches_floyd_reference_through_ply_twelve() {
    for ply in 3..=12 {
        let variable_count = choose_two(ply).expect("small pair count");
        let family = compact_family(
            &(0..ply).collect::<Vec<_>>(),
            &(0..variable_count).collect::<Vec<_>>(),
            ply as u64,
        );
        let transitivity = TransitivityConstraints::try_new(vec![family.clone()], variable_count)
            .expect("small reference family");
        let constraints =
            ConstraintSet::new(Vec::new(), transitivity, 0).expect("small compact set");

        assert_compact_closure_matches_reference(
            &vec![DOMAIN_BOTH; variable_count],
            &constraints,
            &format!("all open, ply {ply}"),
        );

        let mut ascending_chain = vec![DOMAIN_BOTH; variable_count];
        let mut descending_chain = vec![DOMAIN_BOTH; variable_count];
        for face in 0..ply - 1 {
            let variable = family.pair_variable(face, face + 1).expect("adjacent pair");
            ascending_chain[variable] = DOMAIN_FALSE;
            descending_chain[variable] = DOMAIN_TRUE;
        }
        assert_compact_closure_matches_reference(
            &ascending_chain,
            &constraints,
            &format!("ascending chain, ply {ply}"),
        );
        assert_compact_closure_matches_reference(
            &descending_chain,
            &constraints,
            &format!("descending chain, ply {ply}"),
        );

        let mut sparse_order = vec![DOMAIN_BOTH; variable_count];
        let order = (0..ply)
            .filter(|face| face % 2 == 0)
            .chain((0..ply).filter(|face| face % 2 == 1))
            .collect::<Vec<_>>();
        let mut rank = vec![0_usize; ply];
        for (position, face) in order.into_iter().enumerate() {
            rank[face] = position;
        }
        for first in 0..ply {
            for second in first + 1..ply {
                if (first + second) % 3 == 0 {
                    let variable = family.pair_variable(first, second).expect("small pair");
                    sparse_order[variable] = if rank[first] < rank[second] {
                        DOMAIN_FALSE
                    } else {
                        DOMAIN_TRUE
                    };
                }
            }
        }
        assert_compact_closure_matches_reference(
            &sparse_order,
            &constraints,
            &format!("sparse acyclic order, ply {ply}"),
        );

        let mut cycle = vec![DOMAIN_BOTH; variable_count];
        cycle[family.pair_variable(0, 1).expect("cycle pair")] = DOMAIN_FALSE;
        cycle[family.pair_variable(1, 2).expect("cycle pair")] = DOMAIN_FALSE;
        cycle[family.pair_variable(0, 2).expect("cycle pair")] = DOMAIN_TRUE;
        assert_compact_closure_matches_reference(
            &cycle,
            &constraints,
            &format!("fixed cycle, ply {ply}"),
        );
    }

    let transitivity = TransitivityConstraints::try_new(
        vec![
            compact_family(&[0, 1, 2, 3], &[0, 1, 2, 4, 5, 7], 101),
            compact_family(&[1, 2, 3, 4], &[4, 5, 6, 7, 8, 9], 102),
        ],
        10,
    )
    .expect("overlapping reference families");
    let constraints =
        ConstraintSet::new(Vec::new(), transitivity, 0).expect("overlapping compact set");
    let mut overlap_chain = vec![DOMAIN_BOTH; 10];
    for variable in [0, 4, 7, 9] {
        overlap_chain[variable] = DOMAIN_FALSE;
    }
    assert_compact_closure_matches_reference(
        &overlap_chain,
        &constraints,
        "overlapping family chain",
    );
}

#[test]
fn incremental_compact_closure_matches_floyd_across_word_boundaries() {
    for ply in [63_usize, 64, 65, 89, 97] {
        let variable_count = choose_two(ply).expect("bounded pair count");
        let family = compact_family(
            &(0..ply).collect::<Vec<_>>(),
            &(0..variable_count).collect::<Vec<_>>(),
            200 + ply as u64,
        );
        let transitivity = TransitivityConstraints::try_new(vec![family.clone()], variable_count)
            .expect("word-boundary family");
        let constraints =
            ConstraintSet::new(Vec::new(), transitivity, 0).expect("word-boundary set");

        let mut chain = vec![DOMAIN_BOTH; variable_count];
        for face in 0..ply - 1 {
            chain[family
                .pair_variable(face, face + 1)
                .expect("word-boundary chain pair")] = DOMAIN_FALSE;
        }
        assert_compact_closure_matches_reference(
            &chain,
            &constraints,
            &format!("word-boundary chain, ply {ply}"),
        );

        let mut sparse = vec![DOMAIN_BOTH; variable_count];
        for first in 0..ply {
            for second in first + 1..ply {
                if (first.wrapping_mul(17) + second.wrapping_mul(31)) % 11 == 0 {
                    sparse[family
                        .pair_variable(first, second)
                        .expect("word-boundary sparse pair")] = DOMAIN_FALSE;
                }
            }
        }
        assert_compact_closure_matches_reference(
            &sparse,
            &constraints,
            &format!("word-boundary sparse order, ply {ply}"),
        );
    }
}

#[test]
fn incremental_compact_closure_matches_floyd_for_large_overlapping_families() {
    const GLOBAL_PLY: usize = 90;
    let pair_offset = |first: usize, second: usize| {
        first * (2 * GLOBAL_PLY - first - 1) / 2 + (second - first - 1)
    };
    let family = |faces: Vec<usize>, cell: u64| {
        let pair_variables = faces
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(position, first)| {
                faces[position + 1..]
                    .iter()
                    .copied()
                    .map(move |second| pair_offset(first, second))
            })
            .collect::<Vec<_>>();
        compact_family(&faces, &pair_variables, cell)
    };
    let first = family((0..89).collect(), 401);
    let second = family((1..90).collect(), 402);
    let variable_count = choose_two(GLOBAL_PLY).expect("large overlap pair count");
    let transitivity = TransitivityConstraints::try_new(vec![first, second], variable_count)
        .expect("large overlapping families");
    let constraints =
        ConstraintSet::new(Vec::new(), transitivity, 0).expect("large overlapping set");

    let mut chain = vec![DOMAIN_BOTH; variable_count];
    for face in 0..GLOBAL_PLY - 1 {
        chain[pair_offset(face, face + 1)] = DOMAIN_FALSE;
    }
    assert_compact_closure_matches_reference(&chain, &constraints, "large overlapping chain");

    let mut sparse = vec![DOMAIN_BOTH; variable_count];
    for first in 0..GLOBAL_PLY {
        for second in first + 1..GLOBAL_PLY {
            if (first.wrapping_mul(19) + second.wrapping_mul(23)) % 13 == 0 {
                sparse[pair_offset(first, second)] = DOMAIN_FALSE;
            }
        }
    }
    assert_compact_closure_matches_reference(
        &sparse,
        &constraints,
        "large overlapping sparse order",
    );
}

#[test]
fn incremental_closure_word_trail_restores_the_opposite_sibling_exactly() {
    let family = compact_family(&[0, 1, 2, 3], &[0, 1, 2, 3, 4, 5], 103);
    let transitivity =
        TransitivityConstraints::try_new(vec![family.clone()], 6).expect("rollback family");
    let constraints =
        ConstraintSet::new(Vec::new(), transitivity, 0).expect("rollback compact set");
    let mut candidate = vec![DOMAIN_BOTH; 6];
    let mut trail = Vec::with_capacity(candidate.len());
    let mut worklist =
        CompactExplicitWorklist::try_new(candidate.len(), &constraints, &mut |_, _| {
            ConstraintSolverControl::Continue
        })
        .unwrap_or_else(|_| panic!("empty explicit worklist"));
    let mut closure =
        CompactTransitiveClosure::try_new(candidate.len(), &constraints, &mut |_, _| {
            ConstraintSolverControl::Continue
        })
        .unwrap_or_else(|_| panic!("small maintained closure"));
    let domain_mark = trail.len();
    let closure_mark = closure.word_trail_mark();

    for (first, second) in [(0, 1), (1, 2)] {
        let variable = family.pair_variable(first, second).expect("chain pair");
        trail.push((variable, DOMAIN_BOTH));
        candidate[variable] = DOMAIN_FALSE;
        closure
            .enqueue_fixed_variable(variable)
            .unwrap_or_else(|_| panic!("bounded fixed queue"));
    }
    assert!(matches!(
        closure.propagate(
            &mut candidate,
            &constraints,
            0,
            &mut trail,
            &mut worklist,
            &mut |_, _| ConstraintSolverControl::Continue,
        ),
        CompactPropagationResult::Stable
    ));
    assert_eq!(
        candidate[family.pair_variable(0, 2).expect("implied pair")],
        DOMAIN_FALSE
    );
    assert!(closure.word_trail_mark() > closure_mark);

    assert!(matches!(
        undo_compact_search_domains(
            &mut candidate,
            &mut trail,
            CompactRollbackMark {
                domains: domain_mark,
                closure: closure_mark,
            },
            &mut closure,
            &mut worklist,
            0,
            &mut |_, _| ConstraintSolverControl::Continue,
        ),
        CompactPropagationResult::Stable
    ));
    assert!(candidate.iter().all(|domain| *domain == DOMAIN_BOTH));
    assert!(trail.is_empty());
    assert_eq!(closure.word_trail_mark(), closure_mark);
    assert!(closure.reachability.iter().all(|word| *word == 0));

    for (first, second) in [(0, 1), (1, 2)] {
        let variable = family.pair_variable(first, second).expect("chain pair");
        trail.push((variable, DOMAIN_BOTH));
        candidate[variable] = DOMAIN_TRUE;
        closure
            .enqueue_fixed_variable(variable)
            .unwrap_or_else(|_| panic!("bounded sibling queue"));
    }
    assert!(matches!(
        closure.propagate(
            &mut candidate,
            &constraints,
            0,
            &mut trail,
            &mut worklist,
            &mut |_, _| ConstraintSolverControl::Continue,
        ),
        CompactPropagationResult::Stable
    ));
    assert_eq!(
        candidate[family.pair_variable(0, 2).expect("sibling implied pair")],
        DOMAIN_TRUE
    );
}

#[test]
fn incremental_closure_nested_rollbacks_match_floyd_across_a_word_boundary() {
    const PLY: usize = 65;
    const DEPTH: usize = 20;
    let variable_count = choose_two(PLY).expect("nested rollback pair count");
    let family = compact_family(
        &(0..PLY).collect::<Vec<_>>(),
        &(0..variable_count).collect::<Vec<_>>(),
        403,
    );
    let transitivity = TransitivityConstraints::try_new(vec![family.clone()], variable_count)
        .expect("nested rollback family");
    let constraints = ConstraintSet::new(Vec::new(), transitivity, 0).expect("nested rollback set");
    let mut candidate = vec![DOMAIN_BOTH; variable_count];
    let mut trail = Vec::with_capacity(candidate.len());
    let mut worklist =
        CompactExplicitWorklist::try_new(candidate.len(), &constraints, &mut |_, _| {
            ConstraintSolverControl::Continue
        })
        .unwrap_or_else(|_| panic!("empty nested worklist"));
    let mut closure =
        CompactTransitiveClosure::try_new(candidate.len(), &constraints, &mut |_, _| {
            ConstraintSolverControl::Continue
        })
        .unwrap_or_else(|_| panic!("nested maintained closure"));
    let mut frames = Vec::new();

    for face in 0..DEPTH {
        let variable = family
            .pair_variable(face, face + 1)
            .expect("nested chain pair");
        assert_eq!(candidate[variable], DOMAIN_BOTH);
        let mark = CompactRollbackMark {
            domains: trail.len(),
            closure: closure.word_trail_mark(),
        };
        let parent = candidate.clone();
        frames.push((mark, variable, parent));
        trail.push((variable, DOMAIN_BOTH));
        candidate[variable] = DOMAIN_FALSE;
        closure
            .enqueue_fixed_variable(variable)
            .unwrap_or_else(|_| panic!("nested false queue"));
        let mut expected = candidate.clone();
        assert_eq!(
            reference_compact_closure(&mut expected, &constraints),
            CompactClosureOracleOutcome::Stable
        );
        assert!(matches!(
            closure.propagate(
                &mut candidate,
                &constraints,
                0,
                &mut trail,
                &mut worklist,
                &mut |_, _| ConstraintSolverControl::Continue,
            ),
            CompactPropagationResult::Stable
        ));
        assert_eq!(candidate, expected, "false branch at depth {face}");
    }

    while let Some((mark, variable, parent)) = frames.pop() {
        assert!(matches!(
            undo_compact_search_domains(
                &mut candidate,
                &mut trail,
                mark,
                &mut closure,
                &mut worklist,
                0,
                &mut |_, _| ConstraintSolverControl::Continue,
            ),
            CompactPropagationResult::Stable
        ));
        assert_eq!(candidate, parent, "parent restoration for {variable}");
        trail.push((variable, DOMAIN_BOTH));
        candidate[variable] = DOMAIN_TRUE;
        closure
            .enqueue_fixed_variable(variable)
            .unwrap_or_else(|_| panic!("nested true queue"));
        let mut expected = candidate.clone();
        let expected_outcome = reference_compact_closure(&mut expected, &constraints);
        let actual_outcome = match closure.propagate(
            &mut candidate,
            &constraints,
            0,
            &mut trail,
            &mut worklist,
            &mut |_, _| ConstraintSolverControl::Continue,
        ) {
            CompactPropagationResult::Stable => CompactClosureOracleOutcome::Stable,
            CompactPropagationResult::Conflict => CompactClosureOracleOutcome::Conflict,
            _ => CompactClosureOracleOutcome::Invalid,
        };
        assert_eq!(actual_outcome, expected_outcome, "true sibling {variable}");
        if actual_outcome == CompactClosureOracleOutcome::Stable {
            assert_eq!(candidate, expected, "true sibling domains {variable}");
        }
        assert!(matches!(
            undo_compact_search_domains(
                &mut candidate,
                &mut trail,
                mark,
                &mut closure,
                &mut worklist,
                0,
                &mut |_, _| ConstraintSolverControl::Continue,
            ),
            CompactPropagationResult::Stable
        ));
        assert_eq!(candidate, parent, "sibling rollback for {variable}");
    }
}

#[test]
fn maintained_closure_build_and_large_rollback_are_batch_cancelable() {
    const BUILD_PLY: usize = 46;
    let build_variable_count = choose_two(BUILD_PLY).expect("bounded build incidence count");
    let build_transitivity = TransitivityConstraints::try_new(
        vec![compact_family(
            &(0..BUILD_PLY).collect::<Vec<_>>(),
            &(0..build_variable_count).collect::<Vec<_>>(),
            104,
        )],
        build_variable_count,
    )
    .expect("build polling family");
    let build_constraints =
        ConstraintSet::new(Vec::new(), build_transitivity, 0).expect("build polling compact set");
    let mut build_polls = 0_usize;
    assert!(matches!(
        CompactTransitiveClosure::try_new(
            build_variable_count,
            &build_constraints,
            &mut |event, _| {
                if event == ConstraintSolverEvent::PropagationBatch {
                    build_polls += 1;
                    return ConstraintSolverControl::Cancelled;
                }
                ConstraintSolverControl::Continue
            },
        ),
        Err(CompactPropagationResult::Cancelled)
    ));
    assert_eq!(build_polls, 1);

    const ROLLBACK_PLY: usize = 47;
    let rollback_variable_count = choose_two(ROLLBACK_PLY).expect("bounded rollback pair count");
    let rollback_family = compact_family(
        &(0..ROLLBACK_PLY).collect::<Vec<_>>(),
        &(0..rollback_variable_count).collect::<Vec<_>>(),
        105,
    );
    let rollback_transitivity =
        TransitivityConstraints::try_new(vec![rollback_family.clone()], rollback_variable_count)
            .expect("rollback polling family");
    let rollback_constraints = ConstraintSet::new(Vec::new(), rollback_transitivity, 0)
        .expect("rollback polling compact set");
    let mut candidate = vec![DOMAIN_BOTH; rollback_variable_count];
    for face in 0..ROLLBACK_PLY - 1 {
        candidate[rollback_family
            .pair_variable(face, face + 1)
            .expect("rollback chain pair")] = DOMAIN_FALSE;
    }
    let mut trail = Vec::with_capacity(candidate.len());
    let mut worklist =
        CompactExplicitWorklist::try_new(candidate.len(), &rollback_constraints, &mut |_, _| {
            ConstraintSolverControl::Continue
        })
        .unwrap_or_else(|_| panic!("empty rollback worklist"));
    let mut closure =
        CompactTransitiveClosure::try_new(candidate.len(), &rollback_constraints, &mut |_, _| {
            ConstraintSolverControl::Continue
        })
        .unwrap_or_else(|_| panic!("rollback maintained closure"));
    closure
        .enqueue_initial_domains(&candidate, &mut |_, _| ConstraintSolverControl::Continue)
        .unwrap_or_else(|_| panic!("rollback initial queue"));
    assert!(matches!(
        closure.propagate(
            &mut candidate,
            &rollback_constraints,
            0,
            &mut trail,
            &mut worklist,
            &mut |_, _| ConstraintSolverControl::Continue,
        ),
        CompactPropagationResult::Stable
    ));
    assert!(closure.word_trail_mark() > CONTROL_BATCH_RECORDS);
    let mut rollback_polls = 0_usize;
    assert!(matches!(
        closure.rollback(0, 0, &mut |event, _| {
            if event == ConstraintSolverEvent::PropagationBatch {
                rollback_polls += 1;
                return ConstraintSolverControl::Cancelled;
            }
            ConstraintSolverControl::Continue
        }),
        CompactPropagationResult::Cancelled
    ));
    assert_eq!(rollback_polls, 1);
}

fn mixed_compact_constraints() -> ConstraintSet {
    let explicit = vec![
        TupleConstraint {
            kind: FacewiseConstraintKind::Antisymmetry,
            variables: vec![0],
            allowed_rows: vec![0, 1],
            faces: vec![10, 20],
            supporting_cell: None,
        },
        TupleConstraint {
            kind: FacewiseConstraintKind::TacoTortilla,
            variables: vec![0, 1],
            allowed_rows: vec![0, 3],
            faces: vec![10, 20, 30],
            supporting_cell: Some(OverlapCellKey([9; 32])),
        },
        TupleConstraint {
            kind: FacewiseConstraintKind::MountainValley,
            variables: vec![2],
            allowed_rows: vec![0],
            faces: vec![20, 30],
            supporting_cell: None,
        },
    ];
    let transitivity =
        TransitivityConstraints::try_new(vec![compact_family(&[10, 20, 30], &[0, 1, 2], 7)], 3)
            .expect("valid compact family");
    ConstraintSet::new(explicit, transitivity, 1).expect("valid mixed set")
}

fn owned_constraint_signature(
    constraint: ConstraintView<'_>,
) -> (
    FacewiseConstraintKind,
    Vec<usize>,
    Vec<u8>,
    Vec<usize>,
    Option<OverlapCellKey>,
) {
    (
        constraint.kind(),
        constraint.variables().to_vec(),
        constraint.allowed_rows().to_vec(),
        constraint.faces().to_vec(),
        constraint.supporting_cell(),
    )
}

#[test]
fn compact_transitivity_preserves_expanded_global_order_and_conflict_locator() {
    let constraints = mixed_compact_constraints();
    let actual = constraints
        .try_iter()
        .expect("small iterator allocates")
        .map(owned_constraint_signature)
        .collect::<Vec<_>>();
    assert_eq!(constraints.len(), 4);
    assert_eq!(actual[0].0, FacewiseConstraintKind::Antisymmetry);
    assert_eq!(actual[1].0, FacewiseConstraintKind::Transitivity);
    assert_eq!(actual[1].1, vec![0, 1, 2]);
    assert_eq!(actual[1].2, TRANSITIVITY_ALLOWED_ROWS);
    assert_eq!(actual[1].3, vec![10, 20, 30]);
    assert_eq!(
        actual[1].4,
        Some(OverlapCellKey({
            let mut key = [0_u8; 32];
            key[..8].copy_from_slice(&7_u64.to_be_bytes());
            key
        }))
    );
    assert_eq!(actual[2].0, FacewiseConstraintKind::TacoTortilla);
    assert_eq!(actual[3].0, FacewiseConstraintKind::MountainValley);

    let result = solve_constraints_with_memory(
        3,
        &constraints,
        &[Some(false), Some(true), Some(false)],
        0,
        usize::MAX,
        |_, _| ConstraintSolverControl::Continue,
    );
    let ConstraintSolverResult::Unsatisfied {
        conflict_constraint: Some(conflict),
        search_nodes: 0,
    } = result
    else {
        panic!("the fixed directed cycle must identify its compact constraint: {result:?}");
    };
    assert_eq!(conflict.logical_index, 1);
    assert_eq!(conflict.kind, FacewiseConstraintKind::Transitivity);
    assert_eq!(conflict.faces(), &[10, 20, 30]);
    assert_eq!(conflict.supporting_cell, actual[1].4);
}

#[test]
fn compact_iterator_memory_is_preflighted_at_exact_and_one_short_limits() {
    let constraints = mixed_compact_constraints();
    let fixed = [Some(false), Some(false), Some(false)];
    let required = solver_working_memory_upper_bound(fixed.len())
        .and_then(|base| base.checked_add(constraints.iterator_working_memory_upper_bound()?))
        .expect("small fixture fits usize");
    assert!(!constraints.uses_compact_completion());
    assert_eq!(
        solver_working_memory_upper_bound(fixed.len()),
        Some(independent_solver_memory_bytes(fixed.len()))
    );
    assert_eq!(
        required,
        independent_solver_memory_bytes(fixed.len())
            + constraints
                .iterator_working_memory_upper_bound()
                .expect("bounded compact iterator")
    );
    assert!(matches!(
        solve_constraints_with_memory(3, &constraints, &fixed, 0, required, |_, _| {
            ConstraintSolverControl::Continue
        },),
        ConstraintSolverResult::Satisfied { .. }
    ));
    assert_eq!(
        solve_constraints_with_memory(3, &constraints, &fixed, 0, required - 1, |_, _| {
            ConstraintSolverControl::Continue
        },),
        ConstraintSolverResult::WorkingMemoryLimit { observed: required }
    );
}

#[test]
fn compact_heap_initialization_polls_each_bounded_family_batch() {
    let families = (0..1_025_u64)
        .map(|cell| compact_family(&[0, 1, 2], &[0, 1, 2], cell))
        .collect::<Vec<_>>();
    let transitivity =
        TransitivityConstraints::try_new(families, 3).expect("unique families are valid");
    let constraints =
        ConstraintSet::new(Vec::new(), transitivity, 0).expect("compact-only fixture is valid");
    let mut propagation_polls = 0_usize;
    let result = solve_constraints_with_memory(
        3,
        &constraints,
        &[Some(false); 3],
        0,
        usize::MAX,
        |event, _| {
            if event == ConstraintSolverEvent::PropagationBatch {
                propagation_polls += 1;
                if propagation_polls == 2 {
                    return ConstraintSolverControl::Cancelled;
                }
            }
            ConstraintSolverControl::Continue
        },
    );
    assert_eq!(result, ConstraintSolverResult::Cancelled);
    assert_eq!(propagation_polls, 2);
}

#[test]
fn compact_completion_handles_multiple_families_and_rejects_a_fixed_cycle() {
    let transitivity = TransitivityConstraints::try_new(
        vec![
            compact_family(&[0, 1, 2], &[0, 1, 3], 1),
            compact_family(&[1, 2, 3], &[3, 4, 5], 2),
        ],
        6,
    )
    .expect("overlapping compact families use the global pair registry");
    let constraints =
        ConstraintSet::new(Vec::new(), transitivity, 0).expect("valid compact fixture");
    let mut domains = vec![DOMAIN_BOTH; 6];
    domains[0] = DOMAIN_TRUE;
    let result = try_compact_completion(&domains, &constraints, usize::MAX, &mut |_, _| {
        ConstraintSolverControl::Continue
    });
    let CompactCompletionResult::Satisfied {
        candidate,
        search_nodes: 0,
    } = result
    else {
        panic!("compatible overlapping families must complete canonically");
    };
    assert!(
        candidate
            .iter()
            .all(|domain| matches!(*domain, DOMAIN_FALSE | DOMAIN_TRUE))
    );
    let mut check_scratch = compact_order_scratch(4);
    assert!(matches!(
        compact_candidate_check(
            &candidate,
            &constraints,
            &mut check_scratch,
            0,
            &mut |_, _| ConstraintSolverControl::Continue,
        )
        .expect("control continues"),
        CompactCandidateCheck::Accepts
    ));

    let transitivity =
        TransitivityConstraints::try_new(vec![compact_family(&[0, 1, 2], &[0, 1, 2], 3)], 3)
            .expect("valid triangle family");
    let cycle = ConstraintSet::new(Vec::new(), transitivity, 0).expect("valid compact set");
    assert!(matches!(
        try_compact_completion(
            &[DOMAIN_FALSE, DOMAIN_TRUE, DOMAIN_FALSE],
            &cycle,
            usize::MAX,
            &mut |_, _| ConstraintSolverControl::Continue,
        ),
        CompactCompletionResult::Fallback { search_nodes: 0 }
    ));
}

#[test]
fn compact_completion_search_starts_only_after_an_explicit_rejection() {
    let explicit = vec![TupleConstraint {
        kind: FacewiseConstraintKind::TacoTortilla,
        variables: vec![0, 1],
        allowed_rows: vec![1, 2],
        faces: vec![0, 1, 2],
        supporting_cell: Some(OverlapCellKey([17; 32])),
    }];
    let transitivity = TransitivityConstraints::try_new(
        vec![compact_family(&[0, 1, 2, 3], &[0, 1, 2, 3, 4, 5], 17)],
        6,
    )
    .expect("valid four-face compact family");
    let constraints = ConstraintSet::new(explicit, transitivity, 0).expect("valid witness fixture");

    assert!(matches!(
        try_compact_completion(&[DOMAIN_BOTH; 6], &constraints, 0, &mut |_, _| {
            ConstraintSolverControl::Continue
        },),
        CompactCompletionResult::SearchNodeLimit { observed: 1 }
    ));
    let result = try_compact_completion(&[DOMAIN_BOTH; 6], &constraints, 1, &mut |_, _| {
        ConstraintSolverControl::Continue
    });
    let CompactCompletionResult::Satisfied {
        candidate,
        search_nodes: 1,
    } = result
    else {
        panic!("one witness decision must complete the compact order");
    };
    let mut check_scratch = compact_order_scratch(4);
    assert!(matches!(
        compact_candidate_check(
            &candidate,
            &constraints,
            &mut check_scratch,
            1,
            &mut |_, _| ConstraintSolverControl::Continue,
        )
        .expect("control continues"),
        CompactCandidateCheck::Accepts
    ));
}

#[test]
fn compact_branch_selection_prefers_structural_incidence_and_is_batch_cancelable() {
    let explicit = vec![
        TupleConstraint {
            kind: FacewiseConstraintKind::MountainValley,
            variables: vec![0],
            allowed_rows: vec![0],
            faces: vec![0],
            supporting_cell: None,
        },
        TupleConstraint {
            kind: FacewiseConstraintKind::MountainValley,
            variables: vec![2],
            allowed_rows: vec![0],
            faces: vec![2],
            supporting_cell: None,
        },
        TupleConstraint {
            kind: FacewiseConstraintKind::MountainValley,
            variables: vec![2],
            allowed_rows: vec![1],
            faces: vec![2],
            supporting_cell: None,
        },
    ];
    let constraints = ConstraintSet::from_explicit(explicit);
    let worklist = CompactExplicitWorklist::try_new(3, &constraints, &mut |_, _| {
        ConstraintSolverControl::Continue
    })
    .unwrap_or_else(|_| panic!("small selector worklist"));
    let closure = CompactTransitiveClosure::try_new(3, &constraints, &mut |_, _| {
        ConstraintSolverControl::Continue
    })
    .unwrap_or_else(|_| panic!("empty selector closure"));
    assert_eq!(
        select_compact_branch_variable(&[DOMAIN_BOTH; 3], &worklist, &closure, 0, &mut |_, _| {
            ConstraintSolverControl::Continue
        },)
        .unwrap_or_else(|_| panic!("small selector")),
        Some(2)
    );

    let empty = empty_constraints();
    let empty_worklist =
        CompactExplicitWorklist::try_new(CONTROL_BATCH_RECORDS + 1, &empty, &mut |_, _| {
            ConstraintSolverControl::Continue
        })
        .unwrap_or_else(|_| panic!("empty cancellation worklist"));
    let empty_closure =
        CompactTransitiveClosure::try_new(CONTROL_BATCH_RECORDS + 1, &empty, &mut |_, _| {
            ConstraintSolverControl::Continue
        })
        .unwrap_or_else(|_| panic!("empty cancellation closure"));
    let mut polls = 0_usize;
    assert!(matches!(
        select_compact_branch_variable(
            &vec![DOMAIN_BOTH; CONTROL_BATCH_RECORDS + 1],
            &empty_worklist,
            &empty_closure,
            0,
            &mut |event, _| {
                if event == ConstraintSolverEvent::PropagationBatch {
                    polls += 1;
                    return ConstraintSolverControl::Cancelled;
                }
                ConstraintSolverControl::Continue
            },
        ),
        Err(CompactPropagationResult::Cancelled)
    ));
    assert_eq!(polls, 1);
}

fn sibling_rollback_constraints() -> ConstraintSet {
    let explicit = vec![
        // x OR y
        TupleConstraint {
            kind: FacewiseConstraintKind::TacoTortilla,
            variables: vec![0, 1],
            allowed_rows: vec![1, 2, 3],
            faces: vec![0, 1, 2],
            supporting_cell: None,
        },
        // x OR NOT y. With x=false the two tables force opposite y
        // values; the canonical false branch must be rolled back before
        // the satisfiable x=true sibling is tried.
        TupleConstraint {
            kind: FacewiseConstraintKind::TacoTortilla,
            variables: vec![0, 1],
            allowed_rows: vec![0, 1, 3],
            faces: vec![0, 1, 2],
            supporting_cell: None,
        },
    ];
    let transitivity = TransitivityConstraints::try_new(
        vec![compact_family(&[0, 1, 2, 3], &[0, 1, 2, 3, 4, 5], 18)],
        6,
    )
    .expect("valid sibling-rollback family");
    ConstraintSet::new(explicit, transitivity, 0).expect("valid sibling-rollback fixture")
}

#[test]
fn compact_completion_rolls_back_the_first_branch_and_obeys_exact_node_limits() {
    let constraints = sibling_rollback_constraints();
    let mut search_events = Vec::new();
    assert!(matches!(
        try_compact_completion(&[DOMAIN_BOTH; 6], &constraints, 1, &mut |event, nodes| {
            if event == ConstraintSolverEvent::SearchNode {
                search_events.push(nodes);
            }
            ConstraintSolverControl::Continue
        },),
        CompactCompletionResult::SearchNodeLimit { observed: 2 }
    ));
    assert_eq!(search_events, [1, 2]);

    let result = try_compact_completion(&[DOMAIN_BOTH; 6], &constraints, 2, &mut |_, _| {
        ConstraintSolverControl::Continue
    });
    let CompactCompletionResult::Satisfied {
        candidate,
        search_nodes: 2,
    } = result
    else {
        panic!("the true sibling must succeed after exact rollback");
    };
    assert_eq!(candidate[0], DOMAIN_TRUE);
}

#[test]
fn compact_completion_search_reports_cancel_and_deadline_at_the_first_node() {
    let constraints = sibling_rollback_constraints();
    for (abort, expected) in [
        (
            ConstraintSolverControl::Cancelled,
            CompactCompletionResult::Cancelled,
        ),
        (
            ConstraintSolverControl::DeadlineReached,
            CompactCompletionResult::DeadlineReached { search_nodes: 1 },
        ),
    ] {
        let result = try_compact_completion(
            &[DOMAIN_BOTH; 6],
            &constraints,
            usize::MAX,
            &mut |event, _| {
                if event == ConstraintSolverEvent::SearchNode {
                    abort
                } else {
                    ConstraintSolverControl::Continue
                }
            },
        );
        assert_eq!(result, expected);
    }
}

fn exhaustive_two_variable_unsatisfied_constraints() -> Vec<TupleConstraint> {
    vec![
        TupleConstraint {
            kind: FacewiseConstraintKind::TacoTortilla,
            variables: vec![0, 1],
            allowed_rows: vec![1, 2, 3],
            faces: vec![0, 1, 2],
            supporting_cell: None,
        },
        TupleConstraint {
            kind: FacewiseConstraintKind::TacoTortilla,
            variables: vec![0, 1],
            allowed_rows: vec![0, 1, 3],
            faces: vec![0, 1, 2],
            supporting_cell: None,
        },
        TupleConstraint {
            kind: FacewiseConstraintKind::TacoTortilla,
            variables: vec![0, 1],
            allowed_rows: vec![0, 2, 3],
            faces: vec![0, 1, 2],
            supporting_cell: None,
        },
        TupleConstraint {
            kind: FacewiseConstraintKind::TacoTortilla,
            variables: vec![0, 1],
            allowed_rows: vec![0, 1, 2],
            faces: vec![0, 1, 2],
            supporting_cell: None,
        },
    ]
}

#[test]
fn compact_exhaustion_falls_back_without_resetting_search_nodes() {
    const PLY: usize = 86;
    let variable_count = choose_two(PLY).expect("bounded large family");
    let transitivity = TransitivityConstraints::try_new(
        vec![compact_family(
            &(0..PLY).collect::<Vec<_>>(),
            &(0..variable_count).collect::<Vec<_>>(),
            19,
        )],
        variable_count,
    )
    .expect("valid large compact family");
    let explicit = exhaustive_two_variable_unsatisfied_constraints();
    let constraints = ConstraintSet::new(explicit.clone(), transitivity, explicit.len())
        .expect("valid exhaustive fixture");
    assert!(constraints.uses_compact_completion());

    assert!(matches!(
        try_compact_completion(
            &vec![DOMAIN_BOTH; variable_count],
            &constraints,
            usize::MAX,
            &mut |_, _| ConstraintSolverControl::Continue,
        ),
        CompactCompletionResult::Fallback { search_nodes: 2 }
    ));
    let mut search_events = Vec::new();
    let result = solve_constraints_with_memory(
        variable_count,
        &constraints,
        &vec![None; variable_count],
        2,
        usize::MAX,
        |event, nodes| {
            if event == ConstraintSolverEvent::SearchNode {
                search_events.push(nodes);
            }
            ConstraintSolverControl::Continue
        },
    );
    assert_eq!(
        result,
        ConstraintSolverResult::SearchNodeLimit { observed: 3 }
    );
    assert_eq!(search_events, [1, 2, 3]);
}

#[test]
fn compact_all_fixed_explicit_rejection_and_partial_cycle_fall_back() {
    let explicit = vec![TupleConstraint {
        kind: FacewiseConstraintKind::TacoTortilla,
        variables: vec![0, 1],
        allowed_rows: vec![1, 2, 3],
        faces: vec![0, 1, 2],
        supporting_cell: None,
    }];
    let transitivity =
        TransitivityConstraints::try_new(vec![compact_family(&[0, 1, 2], &[0, 1, 2], 20)], 3)
            .expect("valid triangle family");
    let constraints =
        ConstraintSet::new(explicit, transitivity, 0).expect("valid all-fixed rejection fixture");
    assert!(matches!(
        try_compact_completion(&[DOMAIN_FALSE; 3], &constraints, usize::MAX, &mut |_, _| {
            ConstraintSolverControl::Continue
        },),
        CompactCompletionResult::Fallback { search_nodes: 0 }
    ));

    let transitivity =
        TransitivityConstraints::try_new(vec![compact_family(&[0, 1, 2], &[0, 1, 2], 21)], 3)
            .expect("valid triangle family");
    let cycle = ConstraintSet::new(Vec::new(), transitivity, 0).expect("valid cycle fixture");
    assert!(matches!(
        try_compact_completion(
            &[DOMAIN_FALSE, DOMAIN_TRUE, DOMAIN_FALSE],
            &cycle,
            usize::MAX,
            &mut |_, _| ConstraintSolverControl::Continue,
        ),
        CompactCompletionResult::Fallback { search_nodes: 0 }
    ));
}

#[test]
fn overlapping_family_write_fallback_restores_every_completion_domain() {
    let transitivity = TransitivityConstraints::try_new(
        vec![
            compact_family(&[0, 1, 2], &[0, 1, 2], 22),
            compact_family(&[0, 1, 3], &[0, 1, 3], 23),
        ],
        6,
    )
    .expect("individually valid overlapping families");
    let constraints =
        ConstraintSet::new(Vec::new(), transitivity, 0).expect("valid overlapping-family fixture");
    let original = vec![
        DOMAIN_BOTH,
        DOMAIN_TRUE,
        DOMAIN_TRUE,
        DOMAIN_FALSE,
        DOMAIN_BOTH,
        DOMAIN_BOTH,
    ];
    let mut candidate = original.clone();
    let mut scratch = compact_order_scratch(3);
    let mut trail = Vec::with_capacity(candidate.len());
    let mark = trail.len();
    assert!(matches!(
        complete_compact_candidate_orders(
            &mut candidate,
            &constraints,
            &mut scratch,
            &mut trail,
            0,
            &mut |_, _| ConstraintSolverControl::Continue,
        ),
        Ok(CompactOrderResult::Fallback)
    ));
    undo_domains(&mut candidate, &mut trail, mark);
    assert_eq!(candidate, original);
    assert!(trail.is_empty());
}

#[test]
fn large_compact_completion_avoids_cubic_dfs_and_preflights_all_scratch() {
    const PLY: usize = 86;
    let variable_count = choose_two(PLY).expect("bounded pair count");
    let transitivity = TransitivityConstraints::try_new(
        vec![compact_family(
            &(0..PLY).collect::<Vec<_>>(),
            &(0..variable_count).collect::<Vec<_>>(),
            11,
        )],
        variable_count,
    )
    .expect("large compact family is valid");
    let explicit = vec![TupleConstraint {
        kind: FacewiseConstraintKind::MountainValley,
        variables: vec![0],
        allowed_rows: vec![0],
        faces: vec![0],
        supporting_cell: None,
    }];
    let constraints =
        ConstraintSet::new(explicit, transitivity, 0).expect("large compact set is valid");
    assert!(constraints.uses_compact_completion());
    let fixed = vec![None; variable_count];
    let required = solver_working_memory_upper_bound(variable_count)
        .and_then(|base| base.checked_add(constraints.iterator_working_memory_upper_bound()?))
        .and_then(|base| {
            base.checked_add(
                constraints.compact_completion_working_memory_upper_bound(variable_count)?,
            )
        })
        .expect("bounded solver memory");
    assert_eq!(
        constraints.compact_completion_working_memory_upper_bound(variable_count),
        Some(independent_compact_memory_bytes(
            &[PLY],
            variable_count,
            constraints.explicit.len(),
            constraints.compact_explicit_len,
            constraints.compact_explicit_incidence_len,
        ))
    );
    assert_eq!(
        required,
        independent_solver_memory_bytes(variable_count)
            + constraints
                .iterator_working_memory_upper_bound()
                .expect("bounded compact iterator")
            + independent_compact_memory_bytes(
                &[PLY],
                variable_count,
                constraints.explicit.len(),
                constraints.compact_explicit_len,
                constraints.compact_explicit_incidence_len,
            )
    );
    assert_eq!(
        solve_constraints_with_memory(
            variable_count,
            &constraints,
            &fixed,
            0,
            required - 1,
            |_, _| ConstraintSolverControl::Continue,
        ),
        ConstraintSolverResult::WorkingMemoryLimit { observed: required }
    );
    let result =
        solve_constraints_with_memory(variable_count, &constraints, &fixed, 0, required, |_, _| {
            ConstraintSolverControl::Continue
        });
    let ConstraintSolverResult::Satisfied {
        assignment,
        search_nodes: 0,
    } = result
    else {
        panic!("large canonical completion must bypass DFS: {result:?}");
    };
    assert_eq!(assignment.len(), variable_count);
    assert!(assignment.iter().all(|value| !*value));
}

#[test]
fn compact_completion_observes_control_during_quadratic_work() {
    let transitivity = TransitivityConstraints::try_new(
        vec![compact_family(&[0, 1, 2, 3], &[0, 1, 2, 3, 4, 5], 13)],
        6,
    )
    .expect("valid compact family");
    let constraints = ConstraintSet::new(Vec::new(), transitivity, 0).expect("valid compact set");
    assert!(matches!(
        try_compact_completion(&[DOMAIN_BOTH; 6], &constraints, usize::MAX, &mut |_, _| {
            ConstraintSolverControl::Cancelled
        },),
        CompactCompletionResult::Cancelled
    ));
}

#[test]
fn compact_copy_scratch_reset_and_reachability_zeroing_are_batch_pollable() {
    let empty = empty_constraints();
    let mut copy_polls = 0_usize;
    assert_eq!(
        try_compact_completion(
            &vec![DOMAIN_BOTH; CONTROL_BATCH_RECORDS + 1],
            &empty,
            usize::MAX,
            &mut |event, _| {
                if event == ConstraintSolverEvent::PropagationBatch {
                    copy_polls += 1;
                    if copy_polls == 2 {
                        return ConstraintSolverControl::Cancelled;
                    }
                }
                ConstraintSolverControl::Continue
            },
        ),
        CompactCompletionResult::Cancelled
    );
    assert_eq!(copy_polls, 2, "entry poll plus one copy batch");

    let mut values = Vec::with_capacity(CONTROL_BATCH_RECORDS + 1);
    let mut pending = 0_usize;
    let mut reset_polls = 0_usize;
    assert_eq!(
        reset_compact_scratch(
            &mut values,
            CONTROL_BATCH_RECORDS + 1,
            0_u8,
            0,
            &mut pending,
            &mut |_, _| {
                reset_polls += 1;
                ConstraintSolverControl::Cancelled
            },
        ),
        Err(ConstraintSolverControl::Cancelled)
    );
    assert_eq!(reset_polls, 1);

    const PLY: usize = 257;
    let variable_count = choose_two(PLY).expect("bounded reachability fixture");
    let transitivity = TransitivityConstraints::try_new(
        vec![compact_family(
            &(0..PLY).collect::<Vec<_>>(),
            &(0..variable_count).collect::<Vec<_>>(),
            24,
        )],
        variable_count,
    )
    .expect("valid reachability polling family");
    let constraints =
        ConstraintSet::new(Vec::new(), transitivity, 0).expect("valid polling fixture");
    let mut candidate = vec![DOMAIN_BOTH; variable_count];
    let mut trail = Vec::with_capacity(variable_count);
    let mut reachability = Vec::with_capacity(
        compact_reachability_word_count(PLY).expect("bounded reachability words"),
    );
    let mut reachability_polls = 0_usize;
    assert!(matches!(
        propagate_compact_transitivity_reference(
            &mut candidate,
            &constraints,
            0,
            &mut trail,
            &mut reachability,
            &mut |_, _| {
                reachability_polls += 1;
                ConstraintSolverControl::Cancelled
            },
        ),
        Err(ConstraintSolverControl::Cancelled)
    ));
    assert_eq!(reachability_polls, 1);
}

#[test]
fn complete_assignment_verifier_is_quadratic_pollable_and_exactly_bounded() {
    let transitivity = TransitivityConstraints::try_new(
        vec![compact_family(&[0, 1, 2, 3], &[0, 1, 2, 3, 4, 5], 21)],
        6,
    )
    .expect("valid compact family");
    let constraints = ConstraintSet::new(Vec::new(), transitivity, 0).expect("valid compact set");
    let required = complete_assignment_verification_working_memory_upper_bound(&constraints)
        .expect("small verifier scratch");
    assert_eq!(
        verify_complete_assignment_with_memory(&[false; 6], &constraints, required, |_, _| {
            ConstraintSolverControl::Continue
        },),
        CompleteAssignmentVerificationResult::Accepts
    );
    assert_eq!(
        verify_complete_assignment_with_memory(
            &[false, true, false, false, false, false],
            &constraints,
            required,
            |_, _| ConstraintSolverControl::Continue,
        ),
        CompleteAssignmentVerificationResult::Rejects
    );
    assert_eq!(
        verify_complete_assignment_with_memory(&[false; 5], &constraints, required, |_, _| {
            ConstraintSolverControl::Continue
        },),
        CompleteAssignmentVerificationResult::InvalidConstraint
    );
    assert_eq!(
        verify_complete_assignment_with_memory(&[false; 6], &constraints, required - 1, |_, _| {
            ConstraintSolverControl::Continue
        },),
        CompleteAssignmentVerificationResult::WorkingMemoryLimit { observed: required }
    );
    assert_eq!(
        verify_complete_assignment_with_memory(&[false; 6], &constraints, required - 1, |_, _| {
            ConstraintSolverControl::Cancelled
        },),
        CompleteAssignmentVerificationResult::Cancelled
    );
}
