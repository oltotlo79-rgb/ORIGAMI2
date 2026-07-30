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
    for (count, fin_count) in [(9_u8, 4_u8), (10, 5), (11, 6)] {
        let aggregate_parts = vec![
            ori_domain::BeginnerTargetPartRecordV1 {
                kind: ori_domain::BeginnerTargetPartKindV1::Head,
                count: 1,
            },
            ori_domain::BeginnerTargetPartRecordV1 {
                kind: ori_domain::BeginnerTargetPartKindV1::Torso,
                count: 1,
            },
            ori_domain::BeginnerTargetPartRecordV1 {
                kind: ori_domain::BeginnerTargetPartKindV1::Fin,
                count: fin_count,
            },
            ori_domain::BeginnerTargetPartRecordV1 {
                kind: ori_domain::BeginnerTargetPartKindV1::Tail,
                count: 3,
            },
            ori_domain::BeginnerTargetPartRecordV1 {
                kind: ori_domain::BeginnerTargetPartKindV1::Ear,
                count: 2,
            },
        ];
        let aggregate = derive_reference_model_suggestion_v1(
            AssetId::new(),
            &geometry,
            Some(ori_domain::BeginnerTargetCategoryV1::Animal),
            &aggregate_parts,
        )
        .expect("bounded aggregate general GLB suggestion");
        assert_eq!(
            aggregate
                .protrusions
                .iter()
                .map(|target| target.count)
                .collect::<Vec<_>>(),
            vec![fin_count, 3, 2],
            "physical records must retain the authority-significant semantic feature order"
        );
        assert_eq!(
            aggregate.protrusions[0].symmetry,
            if fin_count % 2 == 0 {
                ori_domain::BeginnerProtrusionSymmetryV1::Bilateral
            } else {
                ori_domain::BeginnerProtrusionSymmetryV1::Radial
            }
        );
        assert_eq!(
            aggregate.protrusions[1].symmetry,
            ori_domain::BeginnerProtrusionSymmetryV1::Radial
        );
        assert_eq!(
            aggregate.protrusions[2].symmetry,
            ori_domain::BeginnerProtrusionSymmetryV1::Bilateral
        );
        assert!(aggregate.surface_ranges.len() >= 2);
        let assignments = aggregate
            .protrusions
            .iter()
            .take(2)
            .zip(aggregate.surface_ranges.iter())
            .map(|(target, range)| BeginnerReferenceSurfaceAssignmentV1 {
                range_id: range.id,
                protrusion_id: target.id,
            })
            .collect::<Vec<_>>();
        let retained = reference_model_profile_protrusions_after_surface_selection_v1(
            &assignments,
            &aggregate,
        )
        .expect("surface subset keeps the complete measured protrusion family");
        assert_eq!(retained, aggregate.protrusions);
        assert_eq!(
            retained.iter().map(|target| target.count).sum::<u8>(),
            count,
            "surface/bulge selection must not truncate aggregate semantic endpoints"
        );
        let aggregate_constraints = ori_domain::BeginnerGenerationConstraintsV1 {
            target_category: Some(ori_domain::BeginnerTargetCategoryV1::Animal),
            target_parts: aggregate_parts,
            protrusions: retained,
            ..ori_domain::BeginnerGenerationConstraintsV1::default()
        };
        assert_eq!(
            ori_domain::estimate_symmetric_parameters_v1(&aggregate_constraints)
                .map(|estimate| estimate.protrusion_count),
            Some(count)
        );
    }
    let noncanonical_semantic_order = [
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Head,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Torso,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Tail,
            count: 3,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Fin,
            count: 8,
        },
    ];
    let order_preserving = derive_reference_model_suggestion_v1(
        AssetId::new(),
        &geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Animal),
        &noncanonical_semantic_order,
    )
    .expect("valid noncanonical semantic order");
    assert_eq!(
        order_preserving
            .protrusions
            .iter()
            .map(|target| target.count)
            .collect::<Vec<_>>(),
        vec![3, 8],
        "derive must never silently swap Fin/Tail semantic ownership"
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
fn reference_model_general_counts_two_through_fourteen_keep_all_endpoints() {
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
    for count in 2_u8..=ori_domain::MAX_BEGINNER_GENERAL_PROTRUSION_COUNT_V1 {
        let (fin_count, secondary_kind, secondary_count) = match count {
            2 => (1, ori_domain::BeginnerTargetPartKindV1::Tail, 1),
            3 => (2, ori_domain::BeginnerTargetPartKindV1::Horn, 1),
            _ => {
                let fin_count = count.saturating_sub(1).min(8);
                (
                    fin_count,
                    ori_domain::BeginnerTargetPartKindV1::Tail,
                    count - fin_count,
                )
            }
        };
        let target_parts = vec![
            ori_domain::BeginnerTargetPartRecordV1 {
                kind: ori_domain::BeginnerTargetPartKindV1::Head,
                count: 1,
            },
            ori_domain::BeginnerTargetPartRecordV1 {
                kind: ori_domain::BeginnerTargetPartKindV1::Torso,
                count: 1,
            },
            ori_domain::BeginnerTargetPartRecordV1 {
                kind: ori_domain::BeginnerTargetPartKindV1::Fin,
                count: fin_count,
            },
            ori_domain::BeginnerTargetPartRecordV1 {
                kind: secondary_kind,
                count: secondary_count,
            },
        ];
        let suggestion = derive_reference_model_suggestion_v1(
            AssetId::new(),
            &geometry,
            Some(ori_domain::BeginnerTargetCategoryV1::Animal),
            &target_parts,
        )
        .unwrap_or_else(|error| panic!("two-record General({count}) suggestion failed: {error}"));
        assert_eq!(
            suggestion
                .protrusions
                .iter()
                .map(|target| target.count)
                .collect::<Vec<_>>(),
            vec![fin_count, secondary_count]
        );
        let assignments = suggestion
            .protrusions
            .iter()
            .zip(suggestion.surface_ranges.iter())
            .map(|(target, range)| BeginnerReferenceSurfaceAssignmentV1 {
                range_id: range.id,
                protrusion_id: target.id,
            })
            .collect::<Vec<_>>();
        let edits = suggestion
            .surface_ranges
            .iter()
            .take(assignments.len())
            .map(|range| BeginnerReferenceSurfaceEditV1 {
                range_id: range.id,
                base_digest_sha256: range.digest_sha256,
                triangle_indices: range.triangle_indices.clone(),
                bulge_direction_milli: [0, 0, 1000],
                bulge_amount_tenths_mm: 50,
            })
            .collect::<Vec<_>>();
        assert_eq!(assignments.len(), 2);
        assert_eq!(edits.len(), assignments.len());
        assert!(
            assignments
                .iter()
                .zip(&edits)
                .all(|(assignment, edit)| assignment.range_id == edit.range_id),
            "each selected surface must bind exactly one selected bulge edit"
        );
        assert!(reference_model_surface_selection_matches_live_v1(
            &assignments,
            &edits,
            &suggestion,
            &geometry,
        ));
        let retained = reference_model_profile_protrusions_after_surface_selection_v1(
            &assignments,
            &suggestion,
        )
        .unwrap_or_else(|| panic!("General({count}) surface selection must retain every record"));
        assert_eq!(retained, suggestion.protrusions);
        assert_eq!(
            retained.iter().map(|target| target.count).sum::<u8>(),
            count
        );
        let constraints = ori_domain::BeginnerGenerationConstraintsV1 {
            target_category: Some(ori_domain::BeginnerTargetCategoryV1::Animal),
            target_parts,
            protrusions: retained,
            ..ori_domain::BeginnerGenerationConstraintsV1::default()
        };
        assert_eq!(
            ori_domain::beginner_expected_generated_plan_kind_v1(&constraints),
            Some(ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase)
        );
        assert_eq!(
            ori_domain::estimate_symmetric_parameters_v1(&constraints)
                .map(|estimate| estimate.protrusion_count),
            Some(count)
        );
    }

    let single_record_parts = vec![
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Head,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Torso,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Fin,
            count: 4,
        },
    ];
    let single_record = derive_reference_model_suggestion_v1(
        AssetId::new(),
        &geometry,
        Some(ori_domain::BeginnerTargetCategoryV1::Animal),
        &single_record_parts,
    )
    .expect("single-record General(4) reference suggestion");
    assert_eq!(single_record.protrusions.len(), 1);
    assert_eq!(single_record.protrusions[0].count, 4);
    let single_record_constraints = ori_domain::BeginnerGenerationConstraintsV1 {
        target_category: Some(ori_domain::BeginnerTargetCategoryV1::Animal),
        target_parts: single_record_parts,
        protrusions: single_record.protrusions.clone(),
        ..ori_domain::BeginnerGenerationConstraintsV1::default()
    };
    assert_eq!(
        ori_domain::estimate_symmetric_parameters_v1(&single_record_constraints)
            .map(|estimate| estimate.protrusion_count),
        Some(4),
        "the one-record suggestion is valid General(4) semantics before surface confirmation"
    );
    let single_assignment = [BeginnerReferenceSurfaceAssignmentV1 {
        range_id: single_record.surface_ranges[0].id,
        protrusion_id: single_record.protrusions[0].id,
    }];
    let single_edit = [BeginnerReferenceSurfaceEditV1 {
        range_id: single_record.surface_ranges[0].id,
        base_digest_sha256: single_record.surface_ranges[0].digest_sha256,
        triangle_indices: single_record.surface_ranges[0].triangle_indices.clone(),
        bulge_direction_milli: [0, 0, 1000],
        bulge_amount_tenths_mm: 50,
    }];
    assert!(
        !reference_model_surface_selection_matches_live_v1(
            &single_assignment,
            &single_edit,
            &single_record,
            &geometry,
        ),
        "surface confirmation remains fail-closed below the two-selection minimum"
    );
    assert!(
        reference_model_profile_protrusions_after_surface_selection_v1(
            &single_assignment,
            &single_record,
        )
        .is_none(),
        "one physical General(4) record cannot bypass the two-surface confirmation contract"
    );
}

#[test]
fn reference_retained_general_two_and_four_apply_undo_redo_and_reopen() {
    let _serial = serial_beginner_grid_test();
    let json = br#"{"asset":{"version":"2.0"},"buffers":[{"byteLength":60}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":48},{"buffer":0,"byteOffset":48,"byteLength":12}],"accessors":[{"bufferView":0,"componentType":5126,"count":4,"type":"VEC3","min":[-0.02,-0.03,0],"max":[0.02,0.03,0]},{"bufferView":1,"componentType":5123,"count":6,"type":"SCALAR"}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1}]}]}"#;
    let mut json_chunk = json.to_vec();
    while !json_chunk.len().is_multiple_of(4) {
        json_chunk.push(b' ');
    }
    let mut binary_chunk = Vec::new();
    for coordinate in [
        -0.02_f32, -0.03, 0.0, 0.02, -0.03, 0.0, -0.02, 0.03, 0.0, 0.02, 0.03, 0.0,
    ] {
        binary_chunk.extend_from_slice(&coordinate.to_le_bytes());
    }
    for index in [0_u16, 1, 2, 1, 3, 2] {
        binary_chunk.extend_from_slice(&index.to_le_bytes());
    }
    assert!(binary_chunk.len().is_multiple_of(4));
    let total_length = 12 + 8 + json_chunk.len() + 8 + binary_chunk.len();
    let mut glb = Vec::with_capacity(total_length);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(total_length as u32).to_le_bytes());
    glb.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F_534A_u32.to_le_bytes());
    glb.extend_from_slice(&json_chunk);
    glb.extend_from_slice(&(binary_chunk.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E_4942_u32.to_le_bytes());
    glb.extend_from_slice(&binary_chunk);
    ori_formats::validate_reference_glb_v1(&glb).expect("passive two-triangle GLB");
    let geometry =
        ori_formats::read_reference_glb_geometry_v1(&glb).expect("bounded reference geometry");
    assert_eq!(geometry.triangle_indices, [[0, 1, 2], [1, 3, 2]]);

    use ori_domain::BeginnerTargetPartKindV1::{Antenna, Fin, Head, Horn, Tail, Torso};
    for (count, feature_parts) in [
        (2_u8, vec![(Fin, 1), (Tail, 1)]),
        (4_u8, vec![(Fin, 1), (Tail, 1), (Horn, 1), (Antenna, 1)]),
    ] {
        let asset_id = AssetId::new();
        let target_parts = [(Head, 1), (Torso, 1)]
            .into_iter()
            .chain(feature_parts)
            .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
            .collect::<Vec<_>>();
        let mut suggestion = derive_reference_model_suggestion_v1(
            asset_id,
            &geometry,
            Some(ori_domain::BeginnerTargetCategoryV1::Animal),
            &target_parts,
        )
        .unwrap_or_else(|error| panic!("General({count}) retained suggestion failed: {error}"));
        assert_eq!(suggestion.protrusions.len(), usize::from(count));
        assert!(
            suggestion
                .protrusions
                .iter()
                .all(|target| target.count == 1)
        );
        let placements = if count == 2 {
            vec![(-250, -1_000), (250, 1_000)]
        } else {
            vec![(-250, -1_000), (-250, 1_000), (250, 1_000), (250, -1_000)]
        };
        for (target, (y, x_direction)) in suggestion.protrusions.iter_mut().zip(placements) {
            target.length_tenths_mm = 250;
            target.thickness_tenths_mm = 100;
            target.root_width_tenths_mm = None;
            target.tip_width_tenths_mm = None;
            target.local_outline_tenths_mm = None;
            target.position_tenths_mm = [0, y, 0];
            target.direction_milli = [x_direction, 0, 0];
            target.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::None;
            target.priority = 100;
        }
        let assignments = suggestion
            .protrusions
            .iter()
            .take(2)
            .zip(&suggestion.surface_ranges)
            .map(|(target, range)| BeginnerReferenceSurfaceAssignmentV1 {
                range_id: range.id,
                protrusion_id: target.id,
            })
            .collect::<Vec<_>>();
        let edits = suggestion
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
        assert_eq!(assignments.len(), 2);
        assert!(reference_model_surface_selection_matches_live_v1(
            &assignments,
            &edits,
            &suggestion,
            &geometry,
        ));
        let retained = reference_model_profile_protrusions_after_surface_selection_v1(
            &assignments,
            &suggestion,
        )
        .expect("the confirmed surface subset must retain every semantic record");
        assert_eq!(retained, suggestion.protrusions);
        assert_eq!(
            retained.iter().map(|target| target.count).sum::<u8>(),
            count
        );
        if count == 4 {
            let selected_ids = assignments
                .iter()
                .map(|assignment| assignment.protrusion_id)
                .collect::<HashSet<_>>();
            assert_eq!(selected_ids.len(), 2);
            assert_eq!(
                retained
                    .iter()
                    .filter(|target| !selected_ids.contains(&target.id))
                    .count(),
                2,
                "two unselected reference records must survive the confirmed surface subset"
            );
        }

        let mut project = initial_project_state();
        project
            .reference_model_assets
            .push(ori_formats::ProjectReferenceModelAssetV1 {
                id: asset_id,
                bytes: glb.clone(),
            });
        let topology = project
            .editor
            .topology_analysis_input(project.project_id)
            .analyze();
        let face_id = topology
            .simulation_snapshot()
            .and_then(|snapshot| snapshot.faces.first().map(|face| face.id))
            .expect("initial paper face");
        let source_fold_model_fingerprint = project.editor.fold_model_fingerprint_v1();
        let bulge_targets = assignments
            .iter()
            .zip(&edits)
            .enumerate()
            .map(|(index, (assignment, edit))| {
                let range = suggestion
                    .surface_ranges
                    .iter()
                    .find(|range| range.id == assignment.range_id)
                    .unwrap();
                ori_domain::BeginnerBulgeTargetV1 {
                    id: u16::try_from(index + 1).unwrap(),
                    face_ids: vec![face_id],
                    range_min_tenths_mm: range.range_min_tenths_mm,
                    range_max_tenths_mm: range.range_max_tenths_mm,
                    direction_milli: edit.bulge_direction_milli,
                    amount_tenths_mm: edit.bulge_amount_tenths_mm,
                    source_fold_model_fingerprint: source_fold_model_fingerprint.clone(),
                    reference_surface_binding: Some(
                        ori_domain::BeginnerReferenceSurfaceBindingV1 {
                            asset_id,
                            range_id: range.id,
                            protrusion_id: assignment.protrusion_id,
                            triangle_indices: edit.triangle_indices.clone(),
                            range_digest_sha256: edit.base_digest_sha256,
                        },
                    ),
                }
            })
            .collect::<Vec<_>>();
        let mut profile = ori_domain::BeginnerDesignProfileV1::default();
        profile.generation_constraints.target_category =
            Some(ori_domain::BeginnerTargetCategoryV1::Animal);
        profile.generation_constraints.target_parts = target_parts;
        profile.generation_constraints.protrusions = retained.clone();
        profile.generation_constraints.bulge_targets = bulge_targets;
        profile.generation_constraints.target_asset =
            Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel { asset_id });
        profile.reference_surface_landmarks_tenths_mm =
            Some(suggestion.surface_landmarks_tenths_mm.clone());
        assert_eq!(
            ori_domain::beginner_expected_generated_plan_kind_v1(&profile.generation_constraints),
            Some(ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase)
        );
        assert_eq!(
            ori_domain::estimate_symmetric_parameters_v1(&profile.generation_constraints)
                .map(|estimate| estimate.protrusion_count),
            Some(count)
        );
        let retained_profile = profile.clone();
        let point = ori_domain::beginner_parameter_grid_v1()[13];
        let configured = temporary_symmetric_profile_for_grid(&profile, point)
            .unwrap_or_else(|error| panic!("General({count}) reference grid failed: {error}"));
        assert_eq!(profile, retained_profile);
        assert_eq!(configured.generation_constraints.protrusions, retained);
        assert_eq!(
            configured.generation_constraints.bulge_targets,
            retained_profile.generation_constraints.bulge_targets,
            "temporary grid configuration must preserve every selected reference-surface record"
        );
        assert_eq!(
            ori_domain::estimate_symmetric_parameters_v1(&configured.generation_constraints)
                .map(|estimate| estimate.protrusion_count),
            Some(count)
        );

        let plan = grid_template_plan(
            project.project_id,
            project.editor.pattern(),
            &project.editor.paper().boundary_vertices,
            &profile,
            point,
        )
        .unwrap_or_else(|error| panic!("General({count}) reference generation failed: {error:?}"))
        .into_iter()
        .find(|candidate| {
            candidate.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeGenericTargetBase
                && candidate
                    .instruction_codes
                    .last()
                    .is_some_and(|code| code.ends_with(":horizontal"))
        })
        .expect("horizontal retained-reference generic plan");
        assert_eq!(
            radial_corner_support_added_v1(&plan),
            if count == 2 { 4 } else { 2 }
        );
        let applied_plan_skeleton = plan.skeleton_segments.clone();

        let instance_id = project.instance_id;
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let applied = apply_grid_plan_document(
            &mut project,
            instance_id,
            project_id,
            revision,
            plan,
            configured.clone(),
            None,
        )
        .unwrap_or_else(|error| panic!("General({count}) reference apply failed: {error}"));
        let applied_profile = project.editor.beginner_design_profile().clone();
        let mut expected_constraints = configured.generation_constraints.clone();
        expected_constraints.skeleton_segments = applied_plan_skeleton;
        assert_eq!(applied_profile.generation_constraints, expected_constraints);
        assert_eq!(
            applied_profile.reference_surface_landmarks_tenths_mm,
            retained_profile.reference_surface_landmarks_tenths_mm
        );
        assert_eq!(
            ori_domain::estimate_symmetric_parameters_v1(&applied_profile.generation_constraints)
                .map(|estimate| estimate.protrusion_count),
            Some(count)
        );
        let provenance = applied_profile
            .generation_provenance
            .as_ref()
            .expect("retained reference apply provenance");
        assert!(provenance.source_asset_fingerprint.starts_with("sha256:"));
        assert!(provenance.fold_path_certificate_sha256.is_some());
        assert_eq!(
            ori_core::beginner_generation_document_authority_status_v1(
                project.editor.pattern(),
                project.editor.paper(),
                project.editor.beginner_design_profile(),
            ),
            ori_core::BeginnerGenerationDocumentAuthorityStatusV1::Current
        );
        let applied_document = project.document();

        let undone = execute_undo(&mut project, project_id, applied.revision).unwrap();
        assert_eq!(
            ori_core::beginner_generation_document_authority_status_v1(
                project.editor.pattern(),
                project.editor.paper(),
                project.editor.beginner_design_profile(),
            ),
            ori_core::BeginnerGenerationDocumentAuthorityStatusV1::NoProvenance
        );
        execute_redo(&mut project, project_id, undone.revision).unwrap();
        assert_eq!(project.document(), applied_document);
        assert_eq!(
            ori_core::beginner_generation_document_authority_status_v1(
                project.editor.pattern(),
                project.editor.paper(),
                project.editor.beginner_design_profile(),
            ),
            ori_core::BeginnerGenerationDocumentAuthorityStatusV1::Current
        );

        let mut saved = project.document();
        saved.thumbnail_svg = None;
        let bytes = write_project_ori2(&saved).unwrap();
        let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default()).unwrap();
        let reopened = ProjectState::from_valid_document(
            restored,
            PathBuf::from(format!("reference-general-{count}.ori2")),
        );
        let mut reopened_document = reopened.document();
        reopened_document.thumbnail_svg = None;
        assert_eq!(reopened_document, saved);
        assert_eq!(
            reopened.editor.beginner_design_profile(),
            &saved.beginner_design_profile
        );
        assert_eq!(
            ori_core::beginner_generation_document_authority_status_v1(
                reopened.editor.pattern(),
                reopened.editor.paper(),
                reopened.editor.beginner_design_profile(),
            ),
            ori_core::BeginnerGenerationDocumentAuthorityStatusV1::Current
        );
    }
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
    let retained =
        reference_model_profile_protrusions_after_surface_selection_v1(&assignments, &live)
            .expect("two selected bulge ranges retain all three six-leg pairs");
    assert_eq!(retained, live.protrusions);
    assert_eq!(retained.iter().map(|target| target.count).sum::<u8>(), 6);
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
    assert!(
        reference_model_profile_protrusions_after_surface_selection_v1(&duplicate_part, &live,)
            .is_none()
    );
    let mut forged_part = assignments.clone();
    forged_part[1].protrusion_id = u16::MAX;
    assert!(!reference_model_surface_selection_matches_live_v1(
        &forged_part,
        &edits,
        &live,
        &geometry,
    ));
    assert!(
        reference_model_profile_protrusions_after_surface_selection_v1(&forged_part, &live,)
            .is_none()
    );
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
