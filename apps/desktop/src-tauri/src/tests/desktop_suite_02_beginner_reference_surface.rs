#[test]
fn reference_model_six_legs_are_three_individually_bound_pairs() {
    let geometry = ori_formats::ReferenceGlbGeometryV1 {
        positions: vec![
            [-0.02, -0.03, 0.0],
            [0.02, -0.03, 0.0],
            [-0.02, 0.03, 0.0],
            [0.02, 0.03, 0.0],
        ],
        triangle_indices: vec![[0, 1, 2], [1, 3, 2]],
        material_color: [255, 255, 255, 255],
    };
    let suggestion = derive_reference_model_suggestion_v1(
        AssetId::new(),
        &geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Insect),
        &[ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Leg,
            count: 6,
        }],
    )
    .expect("bounded symmetric GLB suggestion");
    assert_eq!(suggestion.protrusions.len(), 3);
    assert_eq!(suggestion.pair_bindings.len(), 3);
    assert!(
        suggestion
            .protrusions
            .windows(2)
            .all(|pair| { pair[0].position_tenths_mm[1] < pair[1].position_tenths_mm[1] })
    );
    for (index, binding) in suggestion.pair_bindings.iter().enumerate() {
        assert_eq!(binding.pair_index, index as u8);
        assert_eq!(binding.protrusion_id, suggestion.protrusions[index].id);
        assert_eq!(
            binding.center_y_tenths_mm,
            suggestion.protrusions[index].position_tenths_mm[1]
        );
    }
    let complete_parts = [
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Wing,
            count: 2,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Antenna,
            count: 2,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Leg,
            count: 6,
        },
    ];
    let asset_id = AssetId::new();
    let complete = derive_reference_model_suggestion_v1(
        asset_id,
        &geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Insect),
        &complete_parts,
    )
    .expect("bounded complete insect GLB suggestion");
    assert_eq!(complete.protrusions.len(), 5);
    assert_eq!(complete.pair_bindings.len(), 5);
    assert!(
        complete
            .pair_bindings
            .iter()
            .enumerate()
            .all(|(index, binding)| binding.pair_index == index as u8
                && binding.protrusion_id == complete.protrusions[index].id)
    );
    let mut signed_zero_geometry = geometry.clone();
    for position in &mut signed_zero_geometry.positions {
        position[2] = -0.0;
    }
    assert_eq!(
        derive_reference_model_suggestion_v1(
            asset_id,
            &signed_zero_geometry,
            Some(ori_domain::BeginnerTargetCategoryV1::Insect),
            &complete_parts,
        )
        .unwrap(),
        complete
    );
    let mut pair_order_aba = complete.clone();
    pair_order_aba.pair_bindings.swap(2, 4);
    assert_ne!(pair_order_aba, complete);

    let mut asymmetric = geometry.clone();
    asymmetric.positions[3][0] = 0.03;
    assert!(
        derive_reference_model_suggestion_v1(
            asset_id,
            &asymmetric,
            Some(ori_domain::BeginnerTargetCategoryV1::Insect),
            &complete_parts,
        )
        .is_err()
    );
    let generic_parts = [
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Leg,
            count: 4,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Fin,
            count: 2,
        },
    ];
    let generic = derive_reference_model_suggestion_v1(
        AssetId::new(),
        &geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Animal),
        &generic_parts,
    )
    .expect("bounded generic GLB suggestion");
    assert_eq!(generic.protrusions.len(), 2);
    assert_eq!(generic.protrusions[0].id, 1);
    assert_eq!(generic.protrusions[0].count, 4);
    assert_eq!(generic.protrusions[1].id, 2);
    assert_eq!(generic.protrusions[1].count, 2);
    let generalized_parts = [
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Leg,
            count: 4,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Wing,
            count: 2,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Tail,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Fin,
            count: 2,
        },
    ];
    let generalized =
        derive_reference_model_suggestion_v1(AssetId::new(), &geometry, None, &generalized_parts)
            .expect("four explicit generic features remain a bounded candidate");
    assert_eq!(generalized.protrusions.len(), 4);
    assert_eq!(
        generalized
            .protrusions
            .iter()
            .map(|target| target.count)
            .collect::<Vec<_>>(),
        vec![4, 2, 1, 2]
    );
    let mut unsupported_generic_parts = generic_parts;
    unsupported_generic_parts[1].count = 8;
    assert!(
        derive_reference_model_suggestion_v1(
            AssetId::new(),
            &geometry,
            Some(ori_domain::BeginnerTargetCategoryV1::Animal),
            &unsupported_generic_parts,
        )
        .is_err()
    );
    let mut duplicate_parts = complete_parts.to_vec();
    duplicate_parts.push(complete_parts[0].clone());
    assert!(
        derive_reference_model_suggestion_v1(
            asset_id,
            &geometry,
            Some(ori_domain::BeginnerTargetCategoryV1::Insect),
            &duplicate_parts,
        )
        .is_err()
    );
    let mut extreme = geometry.clone();
    extreme.positions[0][0] = f32::INFINITY;
    assert!(
        derive_reference_model_suggestion_v1(
            asset_id,
            &extreme,
            Some(ori_domain::BeginnerTargetCategoryV1::Insect),
            &complete_parts,
        )
        .is_err()
    );

    let mut replacement_geometry = geometry.clone();
    replacement_geometry.positions[2][1] = 0.04;
    replacement_geometry.positions[3][1] = 0.04;
    let replacement = derive_reference_model_suggestion_v1(
        asset_id,
        &replacement_geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Insect),
        &complete_parts,
    )
    .expect("replacement GLB suggestion");
    assert_ne!(replacement, complete);
    let tail = derive_reference_model_suggestion_v1(
        AssetId::new(),
        &geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Animal),
        &[ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Tail,
            count: 1,
        }],
    )
    .expect("bounded center-axis tail suggestion");
    assert_eq!(
        tail.suggested_part_kind,
        Some(ori_domain::BeginnerTargetPartKindV1::Tail)
    );
    assert_eq!(tail.protrusions.len(), 1);
    assert_eq!(tail.protrusions[0].count, 1);
    assert_eq!(
        tail.protrusions[0].symmetry,
        ori_domain::BeginnerProtrusionSymmetryV1::None
    );
    assert_eq!(tail.protrusions[0].direction_milli, [1000, 0, 0]);
    assert_eq!(tail.protrusions[0].length_tenths_mm, 200);
    assert_eq!(tail.protrusions[0].position_tenths_mm[1], 0);
    assert!(tail.pair_bindings.is_empty());
    let complete_animal_asset = AssetId::new();
    let complete_animal_parts = [
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Horn,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Tail,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Ear,
            count: 2,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Leg,
            count: 4,
        },
    ];
    let complete_animal = derive_reference_model_suggestion_v1(
        complete_animal_asset,
        &geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Animal),
        &complete_animal_parts,
    )
    .expect("complete animal GLB suggestion");
    assert_eq!(complete_animal.protrusions.len(), 4);
    assert!(reference_model_suggestion_matches_live_v1(
        &complete_animal,
        &complete_animal
    ));
    let mut forged_id = complete_animal.clone();
    forged_id.protrusions[3].id = 99;
    assert!(!reference_model_suggestion_matches_live_v1(
        &forged_id,
        &complete_animal
    ));
    let mut forged_count = complete_animal.clone();
    forged_count.protrusions[3].count = 2;
    assert!(!reference_model_suggestion_matches_live_v1(
        &forged_count,
        &complete_animal
    ));
    let mut pair_order_aba = complete_animal.clone();
    pair_order_aba.pair_bindings.reverse();
    assert!(!reference_model_suggestion_matches_live_v1(
        &pair_order_aba,
        &complete_animal
    ));
    let mut replacement_geometry = geometry.clone();
    replacement_geometry.positions[2][1] = 0.04;
    replacement_geometry.positions[3][1] = 0.04;
    let replacement = derive_reference_model_suggestion_v1(
        complete_animal_asset,
        &replacement_geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Animal),
        &complete_animal_parts,
    )
    .unwrap();
    assert!(!reference_model_suggestion_matches_live_v1(
        &complete_animal,
        &replacement
    ));
    let mut winged_animal_parts = complete_animal_parts.to_vec();
    winged_animal_parts.push(ori_domain::BeginnerTargetPartRecordV1 {
        kind: ori_domain::BeginnerTargetPartKindV1::Wing,
        count: 2,
    });
    let winged_animal = derive_reference_model_suggestion_v1(
        complete_animal_asset,
        &geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Animal),
        &winged_animal_parts,
    )
    .expect("complete winged animal GLB suggestion");
    assert_eq!(winged_animal.protrusions.len(), 5);
    assert_eq!(winged_animal.protrusions[4].id, 5);
    assert_eq!(winged_animal.protrusions[4].count, 2);
    let mut forged_wing = winged_animal.clone();
    forged_wing.protrusions[4].id = 4;
    assert!(!reference_model_suggestion_matches_live_v1(
        &forged_wing,
        &winged_animal
    ));
    let mut duplicate_wing_parts = winged_animal_parts.clone();
    duplicate_wing_parts.push(ori_domain::BeginnerTargetPartRecordV1 {
        kind: ori_domain::BeginnerTargetPartKindV1::Wing,
        count: 2,
    });
    assert!(
        derive_reference_model_suggestion_v1(
            complete_animal_asset,
            &geometry,
            Some(ori_domain::BeginnerTargetCategoryV1::Animal),
            &duplicate_wing_parts,
        )
        .is_err()
    );
    let composite = derive_reference_model_suggestion_v1(
        AssetId::new(),
        &geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Animal),
        &[
            ori_domain::BeginnerTargetPartRecordV1 {
                kind: ori_domain::BeginnerTargetPartKindV1::Tail,
                count: 1,
            },
            ori_domain::BeginnerTargetPartRecordV1 {
                kind: ori_domain::BeginnerTargetPartKindV1::Ear,
                count: 2,
            },
        ],
    )
    .expect("bounded tail-ear suggestion");
    assert_eq!(composite.protrusions.len(), 2);
    assert_eq!(composite.pair_bindings.len(), 1);
    assert_eq!(
        composite.pair_bindings[0].protrusion_id,
        composite.protrusions[1].id
    );
    let horn = derive_reference_model_suggestion_v1(
        AssetId::new(),
        &geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Animal),
        &[ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Horn,
            count: 1,
        }],
    )
    .expect("bounded center-axis horn suggestion");
    assert_eq!(horn.protrusions.len(), 1);
    assert_eq!(horn.protrusions[0].count, 1);
    assert_eq!(
        horn.protrusions[0].symmetry,
        ori_domain::BeginnerProtrusionSymmetryV1::None
    );
    assert_eq!(horn.protrusions[0].direction_milli, [0, -1000, 0]);
    assert_eq!(horn.protrusions[0].length_tenths_mm, 300);
    assert!(horn.pair_bindings.is_empty());
    let antenna = derive_reference_model_suggestion_v1(
        AssetId::new(),
        &geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Insect),
        &[ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Antenna,
            count: 1,
        }],
    )
    .expect("bounded center-axis antenna suggestion");
    assert_eq!(antenna.protrusions.len(), 1);
    assert_eq!(antenna.protrusions[0].count, 1);
    assert_eq!(
        antenna.protrusions[0].symmetry,
        ori_domain::BeginnerProtrusionSymmetryV1::None
    );
    assert_eq!(antenna.protrusions[0].direction_milli, [0, -1000, 0]);
    assert!(antenna.pair_bindings.is_empty());
}

#[test]
fn reference_model_surface_selection_rejects_missing_duplicate_and_forged_ranges() {
    let geometry = ori_formats::ReferenceGlbGeometryV1 {
        positions: vec![
            [-0.02, -0.03, 0.0],
            [0.02, -0.03, 0.0],
            [-0.02, 0.03, 0.0],
            [0.02, 0.03, 0.0],
        ],
        triangle_indices: vec![[0, 1, 2], [1, 3, 2]],
        material_color: [255, 255, 255, 255],
    };
    let live = derive_reference_model_suggestion_v1(
        AssetId::new(),
        &geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Insect),
        &[ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Leg,
            count: 6,
        }],
    )
    .expect("three measured GLB ranges");
    let assignments = vec![
        BeginnerReferenceSurfaceAssignmentV1 {
            range_id: live.surface_ranges[0].id,
            protrusion_id: live.protrusions[0].id,
        },
        BeginnerReferenceSurfaceAssignmentV1 {
            range_id: live.surface_ranges[1].id,
            protrusion_id: live.protrusions[1].id,
        },
    ];
    let edits = live
        .surface_ranges
        .iter()
        .take(2)
        .map(|range| BeginnerReferenceSurfaceEditV1 {
            range_id: range.id,
            base_digest_sha256: range.digest_sha256,
            triangle_indices: range.triangle_indices.clone(),
            bulge_direction_milli: [0, 0, 1_000],
            bulge_amount_tenths_mm: 50,
        })
        .collect::<Vec<_>>();
    assert!(
        live.surface_ranges
            .iter()
            .all(|range| reference_model_surface_range_is_connected_v1(range, &geometry))
    );
    assert!(reference_model_surface_selection_matches_live_v1(
        &assignments,
        &edits,
        &live,
        &geometry,
    ));
    let mut connected_foreign_triangle = edits.clone();
    connected_foreign_triangle[0].triangle_indices.push(1);
    assert!(
        !reference_model_surface_selection_matches_live_v1(
            &assignments,
            &connected_foreign_triangle,
            &live,
            &geometry,
        ),
        "one valid triangle plus a connected triangle outside the measured range is tampering"
    );
    assert!(!reference_model_surface_selection_matches_live_v1(
        &assignments[..1],
        &edits,
        &live,
        &geometry,
    ));
    let mut duplicate = assignments.clone();
    duplicate[1].range_id = duplicate[0].range_id;
    assert!(!reference_model_surface_selection_matches_live_v1(
        &duplicate, &edits, &live, &geometry
    ));
    let mut duplicate_part = assignments.clone();
    duplicate_part[1].protrusion_id = duplicate_part[0].protrusion_id;
    assert!(!reference_model_surface_selection_matches_live_v1(
        &duplicate_part,
        &edits,
        &live,
        &geometry,
    ));
    let mut forged = assignments;
    forged[1].range_id = u16::MAX;
    assert!(!reference_model_surface_selection_matches_live_v1(
        &forged, &edits, &live, &geometry
    ));
    let mut tampered_digest = edits.clone();
    tampered_digest[0].base_digest_sha256[0] ^= 1;
    assert!(!reference_model_surface_selection_matches_live_v1(
        &[
            BeginnerReferenceSurfaceAssignmentV1 {
                range_id: live.surface_ranges[0].id,
                protrusion_id: live.protrusions[0].id
            },
            BeginnerReferenceSurfaceAssignmentV1 {
                range_id: live.surface_ranges[1].id,
                protrusion_id: live.protrusions[1].id
            },
        ],
        &tampered_digest,
        &live,
        &geometry,
    ));
    let disconnected_geometry = ori_formats::ReferenceGlbGeometryV1 {
        positions: vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [10.0, 0.0, 0.0],
            [11.0, 0.0, 0.0],
            [10.0, 1.0, 0.0],
        ],
        triangle_indices: vec![[0, 1, 2], [3, 4, 5]],
        material_color: [255, 255, 255, 255],
    };
    let mut disconnected = live.surface_ranges[0].clone();
    disconnected.triangle_indices = vec![0, 1];
    assert!(!reference_model_surface_range_is_connected_v1(
        &disconnected,
        &disconnected_geometry,
    ));
    let (component_count, bars) = disconnected_glb_stick_tree_v1(&disconnected_geometry)
        .unwrap()
        .unwrap();
    assert_eq!(component_count, 2);
    assert_eq!(bars.len(), 3);
    let mut nine = disconnected_geometry.clone();
    nine.positions.clear();
    nine.triangle_indices.clear();
    for component in 0..9_u32 {
        let base = nine.positions.len() as u32;
        let x = component as f32 * 10.0;
        nine.positions
            .extend([[x, 0.0, 0.0], [x + 1.0, 0.0, 0.0], [x, 1.0, 0.0]]);
        nine.triangle_indices.push([base, base + 1, base + 2]);
    }
    assert_eq!(
        disconnected_glb_stick_tree_v1(&nine),
        Err("reference_model_component_limit".to_owned())
    );
}

#[test]
fn reference_model_surface_connectivity_is_linear_bounded_and_cooperative() {
    const TRIANGLE_COUNT: usize = 39_999;
    let geometry = ori_formats::ReferenceGlbGeometryV1 {
        positions: Vec::new(),
        triangle_indices: (0..TRIANGLE_COUNT as u32)
            .map(|index| [index, index + 1, index + 2])
            .collect(),
        material_color: [255, 255, 255, 255],
    };
    let range = BeginnerReferenceSurfaceRangeV1 {
        id: 1,
        triangle_indices: std::iter::once(0_u32)
            .chain((1..TRIANGLE_COUNT as u32).rev())
            .collect(),
        range_min_tenths_mm: [0; 3],
        range_max_tenths_mm: [1; 3],
        digest_sha256: [0; 32],
    };
    let now = std::time::Instant::now();
    let deadline = now
        .checked_add(Duration::from_secs(30))
        .expect("test deadline");
    let mut measuring = ReferenceModelSurfaceConnectivityControlV1::new(deadline, None, usize::MAX);
    assert!(reference_model_surface_range_is_connected_with_control_v1(
        &range,
        &geometry,
        &mut measuring,
    ));
    let exact_work = usize::MAX - measuring.remaining_work();
    assert!(exact_work > 0);

    let mut exact = ReferenceModelSurfaceConnectivityControlV1::new(deadline, None, exact_work);
    assert!(reference_model_surface_range_is_connected_with_control_v1(
        &range, &geometry, &mut exact,
    ));
    let mut one_short =
        ReferenceModelSurfaceConnectivityControlV1::new(deadline, None, exact_work - 1);
    assert!(!reference_model_surface_range_is_connected_with_control_v1(
        &range,
        &geometry,
        &mut one_short,
    ));

    let mut expired = ReferenceModelSurfaceConnectivityControlV1::new(
        std::time::Instant::now(),
        None,
        usize::MAX,
    );
    assert!(!reference_model_surface_range_is_connected_with_control_v1(
        &range,
        &geometry,
        &mut expired,
    ));
    let cancelled = AtomicBool::new(true);
    let mut cancelled_control =
        ReferenceModelSurfaceConnectivityControlV1::new(deadline, Some(&cancelled), usize::MAX);
    assert!(!reference_model_surface_range_is_connected_with_control_v1(
        &range,
        &geometry,
        &mut cancelled_control,
    ));

    let mut duplicate = range.clone();
    duplicate.triangle_indices[1] = duplicate.triangle_indices[0];
    let mut duplicate_control =
        ReferenceModelSurfaceConnectivityControlV1::new(deadline, None, usize::MAX);
    assert!(!reference_model_surface_range_is_connected_with_control_v1(
        &duplicate,
        &geometry,
        &mut duplicate_control,
    ));
    let mut invalid_index = range;
    invalid_index.triangle_indices[0] = TRIANGLE_COUNT as u32;
    let mut invalid_control =
        ReferenceModelSurfaceConnectivityControlV1::new(deadline, None, usize::MAX);
    assert!(!reference_model_surface_range_is_connected_with_control_v1(
        &invalid_index,
        &geometry,
        &mut invalid_control,
    ));
}

#[test]
fn beginner_grid_progress_is_bounded_and_cancel_is_generation_scoped() {
    let _serial = serial_beginner_grid_test();
    let generation = ProjectId::new();
    let work = Arc::new(BeginnerGridWork::default());
    work.enumerated.store(99, Ordering::Release);
    work.global_checked.store(99, Ordering::Release);
    work.refinement_iterations.store(99, Ordering::Release);
    beginner_grid_work()
        .lock()
        .unwrap()
        .insert(generation, Arc::clone(&work));
    let progress = get_beginner_parameter_grid_progress(generation).unwrap();
    assert_eq!(progress.enumerated_grid_points, 27);
    assert_eq!(progress.global_checked_candidates, 3);
    assert_eq!(progress.refinement_iterations, 24);
    cancel_beginner_parameter_grid(generation).unwrap();
    cancel_beginner_parameter_grid(generation).unwrap();
    assert!(work.cancelled.load(Ordering::Acquire));
    assert_eq!(
        get_beginner_parameter_grid_progress(generation)
            .unwrap()
            .terminal_state,
        "cancelled"
    );
    for _ in 0..10 {
        let replacement = ProjectId::new();
        let replacement_work = Arc::new(BeginnerGridWork::default());
        let mut registry = beginner_grid_work().lock().unwrap();
        for existing in registry.values() {
            existing.terminal.store(2, Ordering::Release);
        }
        registry.retain(|_, existing| existing.terminal.load(Ordering::Acquire) == 0);
        registry.insert(replacement, replacement_work);
    }
    assert_eq!(beginner_grid_work().lock().unwrap().len(), 1);
    beginner_grid_work().lock().unwrap().clear();
}
