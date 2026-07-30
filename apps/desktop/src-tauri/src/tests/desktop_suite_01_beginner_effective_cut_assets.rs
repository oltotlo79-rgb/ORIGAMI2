#[test]
fn beginner_candidate_analysis_cancellation_and_snapshot_aba_are_fail_closed() {
    let project_id = ProjectId::new();
    let mut project = ProjectState::new(CreasePattern::empty());
    project.project_id = project_id;
    let expectation =
        ProjectExpectation::new(project.instance_id, project_id, project.editor.revision());
    let snapshot = capture_beginner_candidate_analysis_snapshot_v1(&project, expectation)
        .expect("bounded analysis snapshot");
    let cancelled = AtomicBool::new(true);
    let plan = ori_domain::BeginnerGeneratedPlanV1 {
        schema_version: 1,
        kind: ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFourLegBase,
        crease_pattern: CreasePattern::empty(),
        instruction_codes: Vec::new(),
        target_parts: Vec::new(),
        skeleton_segments: Vec::new(),
        target_asset: None,
        semantic_landmark_provenance: None,
    };
    let assessment = assess_beginner_generated_plan_with_control_v1(
        project_id,
        project.editor.paper(),
        project.editor.pattern(),
        &plan,
        None,
        std::time::Instant::now() + Duration::from_secs(1),
        Some(&cancelled),
    );
    assert_eq!(
        (
            assessment.apply_allowed,
            assessment.proof_scope,
            assessment.reason
        ),
        (false, "indeterminate", "deadline_exceeded"),
        "a cancellation must prevent assessment success before any candidate is analyzed"
    );

    project.instance_id = ProjectId::new();
    assert!(
        beginner_candidate_snapshot_is_current_v1(&project, &snapshot).is_err(),
        "a reopened instance with a matching persisted document identity must not publish stale analysis"
    );
}

fn effective_cut_request(keys: Vec<[u8; 32]>) -> EffectiveCutReadOnlyRequestV1 {
    EffectiveCutReadOnlyRequestV1 {
        expected_project_instance_id: ProjectId::new(),
        expected_project_id: ProjectId::new(),
        expected_revision: 7,
        expected_fold_model_fingerprint: "a".repeat(64),
        requested_component_keys: keys,
    }
}

#[test]
fn effective_cut_read_only_request_requires_canonical_bounded_selection() {
    assert!(validate_effective_cut_read_only_request_v1(&effective_cut_request(vec![])).is_err());
    assert!(
        validate_effective_cut_read_only_request_v1(&effective_cut_request(vec![[1; 32]; 65]))
            .is_err()
    );
    assert!(
        validate_effective_cut_read_only_request_v1(&effective_cut_request(vec![[1; 32], [1; 32]]))
            .is_err()
    );
    assert!(
        validate_effective_cut_read_only_request_v1(&effective_cut_request(vec![[2; 32], [1; 32]]))
            .is_err()
    );
    let mut malformed = effective_cut_request(vec![[1; 32]]);
    malformed.expected_fold_model_fingerprint = "A".repeat(64);
    assert!(validate_effective_cut_read_only_request_v1(&malformed).is_err());
    assert!(
        validate_effective_cut_read_only_request_v1(&effective_cut_request(vec![[1; 32], [2; 32]]))
            .is_ok()
    );
}

#[test]
fn effective_cut_read_only_response_serializes_only_aggregate_diagnostics() {
    let response = EffectiveCutReadOnlyResponseV1 {
        version: 1,
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 7,
        fold_model_fingerprint: "a".repeat(64),
        effective_snapshot_fingerprint: [1; 32],
        geometry_model_id: "test-geometry",
        geometry_fingerprint: [2; 32],
        pair_observation_model_id: "test-observation",
        pair_observation_fingerprint: [3; 32],
        multi_hinge_gap_model_id: "test-gap",
        multi_hinge_gap_fingerprint: [4; 32],
        source_flat_pair_count: 1,
        separated_pairs: 1,
        touching_pairs: 0,
        shared_hinge_corridor_observed_pairs: 0,
        shared_vertex_corridor_observed_pairs: 0,
        penetrating_pairs: 0,
        indeterminate_pairs: 0,
        multi_hinge_pairs: 0,
        multi_hinge_union_corridor_unproved_pairs: 0,
        authorizes_project_mutation: false,
        authorizes_persistence: false,
        authorizes_simulation_admission: false,
        authorizes_pair_classification: false,
        authorizes_collision_free_classification: false,
        authorizes_pose_solving: false,
        authorizes_material_removal: false,
    };
    let value = serde_json::to_value(response).expect("serialize diagnostic DTO");
    let object = value.as_object().expect("object DTO");
    assert_eq!(object.len(), 28);
    for forbidden in [
        "faceId",
        "edgeId",
        "vertexId",
        "coordinates",
        "boundary",
        "geometry",
    ] {
        assert!(!object.contains_key(forbidden));
    }
    assert!(object.values().all(|value| !value.is_object()));
}

#[test]
fn effective_cut_candidate_response_exposes_only_opaque_aggregate_data() {
    let response = EffectiveCutCandidateListResponseV1 {
        version: 1,
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 7,
        fold_model_fingerprint: "a".repeat(64),
        model_id: "cut_material_component_selection_diagnostic_v1",
        diagnostic_fingerprint: [1; 32],
        total_component_count: 2,
        boundary_component_count: 1,
        candidates: vec![EffectiveCutCandidateV1 {
            component_key: [2; 32],
            owns_original_boundary: false,
            face_count: 1,
            area_square_mm: 10.0,
            closure_component_count: 1,
            closure_face_count: 1,
            nested_dependency_count: 0,
        }],
        authorizes_project_mutation: false,
        authorizes_persistence: false,
        authorizes_simulation_admission: false,
        authorizes_material_removal: false,
    };
    let value = serde_json::to_value(response).expect("serialize candidate DTO");
    let text = serde_json::to_string(&value).expect("candidate JSON");
    for forbidden in [
        "faceId",
        "edgeId",
        "vertexId",
        "boundaryWorld",
        "coordinates",
    ] {
        assert!(!text.contains(forbidden));
    }
    let candidate = value["candidates"][0].as_object().expect("candidate");
    assert_eq!(
        candidate.keys().cloned().collect::<BTreeSet<_>>(),
        [
            "areaSquareMm",
            "closureComponentCount",
            "closureFaceCount",
            "componentKey",
            "faceCount",
            "nestedDependencyCount",
            "ownsOriginalBoundary",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
}

#[test]
fn effective_cut_candidate_fixture_has_stable_nested_read_only_closures() {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("cut-enabled sheet");
    let (mut pattern, paper) = sheet.into_parts();
    for coordinates in [
        (20.0, 20.0),
        (80.0, 20.0),
        (80.0, 80.0),
        (20.0, 80.0),
        (40.0, 40.0),
        (60.0, 40.0),
        (60.0, 60.0),
        (40.0, 60.0),
    ] {
        pattern.vertices.push(Vertex {
            id: VertexId::new(),
            position: Point2::new(coordinates.0, coordinates.1),
        });
    }
    let loop_ids = pattern.vertices[pattern.vertices.len() - 8..]
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    for base in [0, 4] {
        for offset in 0..4 {
            pattern.edges.push(Edge {
                id: EdgeId::new(),
                start: loop_ids[base + offset],
                end: loop_ids[base + (offset + 1) % 4],
                kind: EdgeKind::Cut,
            });
        }
    }
    let editor = EditorState::with_paper(pattern, paper);
    let project_id = ProjectId::new();
    let source = FaceExtractionInput {
        identity_namespace: project_id,
        source_revision: editor.revision(),
        paper: editor.paper(),
        pattern: editor.pattern(),
    };
    let before = editor.fold_model_fingerprint_v1();
    let first = analyze_effective_cut_candidates_v1(source).unwrap();
    let second = analyze_effective_cut_candidates_v1(source).unwrap();
    let mut reordered_pattern = editor.pattern().clone();
    reordered_pattern.vertices.reverse();
    reordered_pattern.edges.reverse();
    let reordered_paper = editor.paper().clone();
    let reordered = analyze_effective_cut_candidates_v1(FaceExtractionInput {
        identity_namespace: project_id,
        source_revision: editor.revision(),
        paper: &reordered_paper,
        pattern: &reordered_pattern,
    })
    .unwrap();
    assert_eq!(first.diagnostic_fingerprint, second.diagnostic_fingerprint);
    assert_eq!(first.model_id, second.model_id);
    assert_eq!(first.total_component_count, second.total_component_count);
    assert_eq!(
        first.boundary_component_count,
        second.boundary_component_count
    );
    assert_eq!(first.candidates, second.candidates);
    assert_eq!(first.model_id, reordered.model_id);
    assert_eq!(
        first.diagnostic_fingerprint,
        reordered.diagnostic_fingerprint
    );
    assert_eq!(first.total_component_count, reordered.total_component_count);
    assert_eq!(
        first.boundary_component_count,
        reordered.boundary_component_count
    );
    assert_eq!(first.candidates, reordered.candidates);
    assert_eq!(first.total_component_count, 3);
    assert_eq!(first.boundary_component_count, 1);
    assert_eq!(first.candidates.len(), 2);
    assert!(
        first
            .candidates
            .iter()
            .all(|candidate| !candidate.owns_original_boundary)
    );
    assert!(
        first
            .candidates
            .iter()
            .any(|candidate| candidate.closure_component_count == 2
                && candidate.nested_dependency_count == 1),
        "outer nested candidate must explain its dependent inner component"
    );
    assert_eq!(editor.fold_model_fingerprint_v1(), before);
    assert!(
        diagnose_cut_material_removal_plan_v1(
            source,
            &[MaterialComponentKey([0xff; 32])],
            Default::default(),
        )
        .is_err()
    );
}

#[test]
fn project_state_preserves_passive_material_void_evidence_from_document() {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let vertices = [
        VertexId::new(),
        VertexId::new(),
        VertexId::new(),
        VertexId::new(),
    ];
    let mut edges = [EdgeId::new(), EdgeId::new(), EdgeId::new(), EdgeId::new()];
    edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    for (id, position) in vertices.into_iter().zip([
        Point2::new(20.0, 20.0),
        Point2::new(30.0, 20.0),
        Point2::new(30.0, 30.0),
        Point2::new(20.0, 30.0),
    ]) {
        pattern.vertices.push(Vertex { id, position });
    }
    for (index, id) in edges.into_iter().enumerate() {
        pattern.edges.push(Edge {
            id,
            start: vertices[index],
            end: vertices[(index + 1) % vertices.len()],
            kind: EdgeKind::Cut,
        });
    }
    let mut document = ProjectDocument::new("Passive void", pattern);
    document.paper = paper;
    let removal_plan_sha256 = [0x31; 32];
    let removed_component_keys = vec![[0x41; 32]];
    let boundary_edge_loop = edges.to_vec();
    let region_id_sha256 = ori_domain::material_void_region_id_sha256_v1(
        removal_plan_sha256,
        &removed_component_keys,
        &boundary_edge_loop,
    );
    let fingerprint =
        ori_core::fold_model_fingerprint_v1(&document.crease_pattern, &document.paper);
    document.material_void_evidence = ori_domain::MaterialVoidEvidenceDocumentV1 {
        version: 1,
        source_project_id: Some(document.project_id),
        source_revision: 8,
        source_fold_model_fingerprint: "c".repeat(64),
        post_fold_model_fingerprint: fingerprint,
        regions: vec![ori_domain::MaterialVoidRegionEvidenceV1 {
            region_id_sha256,
            removal_plan_sha256,
            removed_component_keys,
            boundary_edge_loop,
        }],
    };
    let expected = document.material_void_evidence.clone();
    let project = ProjectState::from_valid_document(document, PathBuf::from("passive-void.ori2"));
    assert_eq!(project.material_void_evidence, expected);
    assert_eq!(project.document().material_void_evidence, expected);
}

#[test]
fn project_state_rejects_invalid_beginner_profile_without_panicking() {
    let mut document = ProjectState::new(CreasePattern::empty()).document();
    document.beginner_design_profile.schema_version = 0;

    let result = ProjectState::from_document(document, PathBuf::from("invalid-profile.ori2"));

    assert_eq!(
        result.err().expect("invalid profile must fail closed"),
        PROJECT_ARCHIVE_INVALID_MESSAGE
    );
}

#[test]
fn bounded_beginner_asset_import_reads_real_regular_file_without_following_aliases() {
    let directory = TestDirectory::new();
    let image = directory.join("target.png");
    fs::write(&image, b"bounded-real-file").expect("fixture");
    assert_eq!(
        read_bounded_regular_import_file(&image, 64, "read", "bounds").unwrap(),
        b"bounded-real-file"
    );
    assert_eq!(
        read_bounded_regular_import_file(&image, 4, "read", "bounds"),
        Err("bounds".to_owned())
    );
    let empty = directory.join("empty.glb");
    fs::write(&empty, []).expect("empty fixture");
    assert_eq!(
        read_bounded_regular_import_file(&empty, 64, "read", "bounds"),
        Err("bounds".to_owned())
    );
    let png_path = directory.join("recognized-target.png");
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("PNG header");
        writer
            .write_image_data(&[
                0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255,
            ])
            .expect("PNG pixels");
    }
    fs::write(&png_path, &png_bytes).expect("PNG fixture");
    let imported_png = read_bounded_regular_import_file(
        &png_path,
        MAX_PROJECT_TEXTURE_ASSET_BYTES,
        "read",
        "bounds",
    )
    .expect("real PNG import");
    assert!(valid_png_image_envelope(&imported_png));
    assert!(beginner_recognition::decode_general_image(&imported_png).is_ok());

    let glb_path = directory.join("multi-component.glb");
    let json = br#"{"asset":{"version":"2.0"}}"#;
    let padded = (json.len() + 3) & !3;
    let total = 12 + 8 + padded;
    let mut glb = Vec::with_capacity(total);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(padded as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F_534A_u32.to_le_bytes());
    glb.extend_from_slice(json);
    glb.resize(total, b' ');
    fs::write(&glb_path, &glb).expect("GLB fixture");
    let imported_glb = read_bounded_regular_import_file(
        &glb_path,
        ori_formats::MAX_REFERENCE_GLB_BYTES_V1,
        "read",
        "bounds",
    )
    .expect("real GLB import");
    ori_formats::validate_reference_glb_v1(&imported_glb).expect("valid passive GLB");

    let mut project = initial_project_state();
    project.texture_assets.push(ProjectTextureAssetV1 {
        id: AssetId::new(),
        media_type: ProjectTextureMediaTypeV1::Png,
        bytes: imported_png,
    });
    project
        .reference_model_assets
        .push(ori_formats::ProjectReferenceModelAssetV1 {
            id: AssetId::new(),
            bytes: imported_glb,
        });
    let saved = project.document();
    let archive = write_project_ori2(&saved).expect("archive imported assets");
    let restored = read_project_ori2_with_limits(&archive, Ori2Limits::default())
        .expect("restore imported assets");
    assert_eq!(restored.texture_assets, saved.texture_assets);
    assert_eq!(
        restored.reference_model_assets,
        saved.reference_model_assets
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let alias = directory.join("alias.png");
        symlink(&image, &alias).expect("alias");
        assert_eq!(
            read_bounded_regular_import_file(&alias, 64, "read", "bounds"),
            Err("bounds".to_owned())
        );
    }
}

#[test]
fn coexisting_beginner_assets_survive_underlay_history_and_reject_stale_overwrite() {
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0, 0, 0, 255]).unwrap();
    }
    let image_a = AssetId::new();
    let image_b = AssetId::new();
    let model = AssetId::new();
    let mut project = initial_project_state();
    for id in [image_a, image_b] {
        project.texture_assets.push(ProjectTextureAssetV1 {
            id,
            media_type: ProjectTextureMediaTypeV1::Png,
            bytes: png_bytes.clone(),
        });
    }
    let json = br#"{"asset":{"version":"2.0"}}"#;
    let padded = (json.len() + 3) & !3;
    let total = 20 + padded;
    let mut glb = b"glTF".to_vec();
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(padded as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F_534A_u32.to_le_bytes());
    glb.extend_from_slice(json);
    glb.resize(total, b' ');
    project
        .reference_model_assets
        .push(ori_formats::ProjectReferenceModelAssetV1 {
            id: model,
            bytes: glb,
        });
    let project_id = project.project_id;
    let layer = ori_domain::LayerId::new();
    execute_command(
        &mut project,
        project_id,
        0,
        Command::CreateLayer {
            layer: LayerRecordV1 {
                id: layer,
                name: "Reference image".to_owned(),
                content_kind: LayerContentKindV1::Underlay,
                visible: true,
                locked: false,
                opacity: 1.0,
            },
            target_index: 1,
        },
    )
    .unwrap();
    let underlay = ori_domain::UnderlayId::new();
    let record = |asset| ori_domain::UnderlayRecordV1 {
        id: underlay,
        asset,
        transform: ori_domain::UnderlayTransformV1 {
            position: Point2::new(0.0, 0.0),
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_degrees: 0.0,
        },
        opacity: 1.0,
        layer,
    };
    let stale_revision = project.editor.revision();
    let added = execute_command(
        &mut project,
        project_id,
        stale_revision,
        Command::AddUnderlay {
            record: record(image_a),
        },
    )
    .unwrap();
    let replaced = execute_command(
        &mut project,
        project_id,
        added.revision,
        Command::UpdateUnderlay {
            record: record(image_b),
        },
    )
    .unwrap();
    let removed = execute_command(
        &mut project,
        project_id,
        replaced.revision,
        Command::RemoveUnderlay { id: underlay },
    )
    .unwrap();
    let undo_remove = execute_undo(&mut project, project_id, removed.revision).unwrap();
    assert_eq!(project.editor.underlays().underlays[0].asset, image_b);
    let undo_replace = execute_undo(&mut project, project_id, undo_remove.revision).unwrap();
    assert_eq!(project.editor.underlays().underlays[0].asset, image_a);
    let redone = execute_redo(&mut project, project_id, undo_replace.revision).unwrap();
    assert_eq!(project.editor.underlays().underlays[0].asset, image_b);

    let before_stale = project.document();
    assert!(
        execute_command(
            &mut project,
            project_id,
            stale_revision,
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(ori_domain::BeginnerDesignProfileV1::default()),
            },
        )
        .is_err()
    );
    assert_eq!(project.document(), before_stale);
    let archive = write_project_ori2(&before_stale).unwrap();
    let restored = read_project_ori2_with_limits(&archive, Ori2Limits::default()).unwrap();
    assert_eq!(restored, before_stale);
    assert_eq!(restored.texture_assets.len(), 2);
    assert_eq!(restored.reference_model_assets[0].id, model);
    assert_eq!(redone.revision, undo_replace.revision + 1);
}

struct BeginnerGridTestGuard {
    _serial: std::sync::MutexGuard<'static, ()>,
}

fn poison_mutex_for_test<T>(mutex: &Mutex<T>) {
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = mutex.lock().expect("unpoisoned registry");
        panic!("poison registry for recovery test");
    }));
    assert!(unwind.is_err());
    assert!(mutex.is_poisoned());
}

fn clear_beginner_work_registries_for_test() {
    let grid_registry = beginner_grid_work();
    grid_registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
    grid_registry.clear_poison();
    let consensus_registry = reference_consensus_work_v1();
    consensus_registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
    consensus_registry.clear_poison();
    beginner_design_commands::clear_beginner_work_generation_tombstones_for_test_v1();
}

fn serial_beginner_grid_test() -> BeginnerGridTestGuard {
    let serial = BEGINNER_GRID_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    BEGINNER_GRID_TEST_LOCK.clear_poison();
    clear_beginner_work_registries_for_test();
    BeginnerGridTestGuard { _serial: serial }
}

impl Drop for BeginnerGridTestGuard {
    fn drop(&mut self) {
        clear_beginner_work_registries_for_test();
    }
}
