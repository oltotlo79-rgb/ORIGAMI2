use std::{
    collections::BTreeSet,
    fs,
    io::{Cursor, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering as AtomicOrdering},
        mpsc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ori_domain::{Edge, LayerContentKindV1, LayerRecordV1, Vertex};
use ori_formats::{
    Ori2Limits, read_project_archive_ori2, read_project_folder_v1, read_project_ori2_with_limits,
    write_project_archive_ori2, write_project_folder_v1, write_project_ori2,
};
#[cfg(target_os = "windows")]
use std::fs::OpenOptions;
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

use super::*;

static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);
static BEGINNER_GRID_TEST_LOCK: Mutex<()> = Mutex::new(());

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

fn serial_beginner_grid_test() -> BeginnerGridTestGuard {
    let serial = BEGINNER_GRID_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    beginner_grid_work()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
    BeginnerGridTestGuard { _serial: serial }
}

impl Drop for BeginnerGridTestGuard {
    fn drop(&mut self) {
        beginner_grid_work()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}

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

#[test]
fn reference_consensus_cancel_is_generation_scoped_and_idempotent() {
    let generation = ProjectId::new();
    let work = Arc::new(ReferenceConsensusWorkV1::default());
    reference_consensus_work_v1()
        .lock()
        .unwrap()
        .insert(generation, Arc::clone(&work));
    cancel_reference_consensus(generation).unwrap();
    cancel_reference_consensus(generation).unwrap();
    assert!(work.cancelled.load(Ordering::Acquire));
    reference_consensus_work_v1()
        .lock()
        .unwrap()
        .remove(&generation);
    assert!(cancel_reference_consensus(generation).is_err());
}

#[test]
fn grid_profile_is_temporary_canonical_and_does_not_change_free_parameters() {
    let _serial = serial_beginner_grid_test();
    let mut source = ori_domain::BeginnerDesignProfileV1::default();
    source.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    source.generation_constraints.target_parts = vec![
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Head,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Torso,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Leg,
            count: 4,
        },
    ];
    let before = source.clone();
    let point = ori_domain::beginner_parameter_grid_v1()[26];
    let temporary = temporary_symmetric_profile_for_grid(&source, point).unwrap();

    assert_eq!(source, before);
    assert_eq!(
        temporary.generation_constraints.detail_level,
        ori_domain::BeginnerDetailLevelV1::Detailed
    );
    assert_eq!(temporary.generation_constraints.protrusions.len(), 1);
    assert_eq!(
        temporary.generation_constraints.protrusions[0].length_tenths_mm,
        450
    );
    assert_eq!(
        temporary.generation_constraints.protrusions[0].thickness_tenths_mm,
        160
    );
    let mut forged = point;
    forged.scale_percent = 44;
    assert_eq!(
        temporary_symmetric_profile_for_grid(&source, forged),
        Err("beginner_parameter_grid_point_invalid".to_owned())
    );
    let mut model_source = source.clone();
    configure_symmetric_profile(
        &mut model_source,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 4,
            scale_percent: 25,
            spacing_percent: 35,
        },
        27,
        50,
    );
    model_source.generation_constraints.protrusions[0].length_tenths_mm = 270;
    model_source.generation_constraints.protrusions[0].thickness_tenths_mm = 100;
    model_source.generation_constraints.target_asset =
        Some(ori_domain::BeginnerTargetAssetReferenceV1::ReferenceModel {
            asset_id: AssetId::new(),
        });
    let model_candidate = temporary_symmetric_profile_for_grid(
        &model_source,
        ori_domain::beginner_parameter_grid_v1()[0],
    )
    .unwrap();
    assert_eq!(
        model_candidate.generation_constraints.protrusions[0].length_tenths_mm,
        100
    );
    assert_eq!(
        model_candidate.generation_constraints.protrusions[0].thickness_tenths_mm,
        40
    );

    let mut project = initial_project_state();
    for point in ori_domain::beginner_parameter_grid_v1() {
        let plans = grid_template_plan(
            project.project_id,
            project.editor.pattern(),
            &project.editor.paper().boundary_vertices,
            &source,
            point,
        )
        .unwrap();
        assert!(!plans.is_empty());
        assert!(plans.len() <= ori_domain::MAX_BEGINNER_GENERATED_CANDIDATES_V1);
    }
    let point = ori_domain::beginner_parameter_grid_v1()[26];
    let plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &source,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|plan| plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFourLegBase)
    .unwrap();
    let project_id = project.project_id;
    let instance_id = project.instance_id;
    let revision = project.editor.revision();
    let snapshot = apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        revision,
        plan.clone(),
    )
    .unwrap();
    assert_eq!(snapshot.revision, revision + 1);
    assert!(
        apply_grid_plan_document(&mut project, instance_id, project_id, revision, plan,).is_err()
    );
    let undone = execute_undo(&mut project, project_id, snapshot.revision).unwrap();
    assert_eq!(undone.revision, snapshot.revision + 1);
    let redone = execute_redo(&mut project, project_id, undone.revision).unwrap();
    assert_eq!(redone.revision, undone.revision + 1);
}

#[test]
fn complete_insect_grid_preserves_all_five_pair_dimensions_and_bindings() {
    let _serial = serial_beginner_grid_test();
    let mut source = ori_domain::BeginnerDesignProfileV1::default();
    source.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Insect);
    source.generation_constraints.target_parts = vec![
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Head,
            count: 1,
        },
        ori_domain::BeginnerTargetPartRecordV1 {
            kind: ori_domain::BeginnerTargetPartKindV1::Torso,
            count: 1,
        },
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
    configure_symmetric_profile(
        &mut source,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 10,
            scale_percent: 27,
            spacing_percent: 50,
        },
        27,
        50,
    );
    for (index, target) in source
        .generation_constraints
        .protrusions
        .iter_mut()
        .enumerate()
    {
        target.length_tenths_mm = if index == 0 {
            1
        } else {
            270 + index as u32 * 27
        };
        target.thickness_tenths_mm = if index == 0 {
            1
        } else {
            50 + index as u16 * 10
        };
        target.direction_milli[0] = -target.direction_milli[0];
        target.direction_milli[1] = -target.direction_milli[1];
    }
    source.generation_constraints.protrusions.reverse();
    let point = ori_domain::beginner_parameter_grid_v1()[26];
    let temporary = temporary_symmetric_profile_for_grid(&source, point).unwrap();

    assert_eq!(temporary.generation_constraints.protrusions.len(), 5);
    assert!(ori_domain::insect_complete_bindings_v1(&temporary.generation_constraints).is_some());
    for (index, target) in temporary
        .generation_constraints
        .protrusions
        .iter()
        .enumerate()
    {
        assert_eq!(target.id, index as u16 + 1);
        let source_length = if index == 0 {
            1
        } else {
            270 + index as u32 * 27
        };
        let source_thickness = if index == 0 {
            1
        } else {
            50 + index as u16 * 10
        };
        assert_eq!(target.length_tenths_mm, (source_length * 45 / 27).max(1));
        assert_eq!(
            target.thickness_tenths_mm,
            (source_thickness * 80 / 50).max(1)
        );
    }

    let mut generatable = source.clone();
    for target in &mut generatable.generation_constraints.protrusions {
        target.length_tenths_mm = 270;
    }
    let mut project = initial_project_state();
    let plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &generatable,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|plan| plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteInsectBase)
    .unwrap();
    let project_id = project.project_id;
    let instance_id = project.instance_id;
    let profile_revision = project.editor.revision();
    let profile_saved = execute_command(
        &mut project,
        project_id,
        profile_revision,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(generatable),
        },
    )
    .unwrap();
    let revision = profile_saved.revision;
    let applied = apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        revision,
        plan.clone(),
    )
    .unwrap();
    let generated_steps = &project.editor.instruction_timeline().steps;
    assert_eq!(generated_steps.len(), 1);
    assert_eq!(
        generated_steps[0].title,
        "Complete composite insect grid candidate"
    );
    assert!(
        apply_grid_plan_document(&mut project, instance_id, project_id, revision, plan).is_err()
    );
    let undone = execute_undo(&mut project, project_id, applied.revision).unwrap();
    let redone = execute_redo(&mut project, project_id, undone.revision).unwrap();
    assert_eq!(redone.revision, undone.revision + 1);
    let saved = project.document();
    let bytes = write_project_ori2(&saved).expect("persist complete insect grid apply");
    let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default())
        .expect("restore complete insect grid apply");
    let reopened =
        ProjectState::from_valid_document(restored, PathBuf::from("complete-insect-grid.ori2"));
    assert_eq!(reopened.document(), saved);
    assert!(
        ori_domain::insect_complete_bindings_v1(
            &reopened
                .editor
                .beginner_design_profile()
                .generation_constraints
        )
        .is_some()
    );
    let score_input = ori_domain::BeginnerCandidateInputV1 {
        vertex_count: project.editor.pattern().vertices.len(),
        edge_count: project.editor.pattern().edges.len(),
        crease_count: project
            .editor
            .pattern()
            .edges
            .iter()
            .filter(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
            .count(),
        target_approximation_score: ori_domain::beginner_target_approximation_score_v1(
            &project
                .editor
                .beginner_design_profile()
                .generation_constraints,
        ),
    };
    assert_eq!(
        ori_domain::score_beginner_candidates_v1(
            score_input,
            project.editor.beginner_design_profile()
        ),
        ori_domain::score_beginner_candidates_v1(
            score_input,
            reopened.editor.beginner_design_profile()
        )
    );
    assert!(!reopened.editor.can_undo());
    assert!(!reopened.editor.can_redo());
}

mod beginner_general_tree;
mod beginner_general_tree_root;

#[test]
fn asymmetric_landmark_native_apply_undo_redo_and_archive_round_trip() {
    let _serial = serial_beginner_grid_test();
    for (plan_kind, target_kind, target_count, archive_name, semantic_binding_count) in [
        (
            ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricBirdLandmarkBase,
            ori_domain::BeginnerTargetPartKindV1::Wing,
            2,
            "asymmetric-bird.ori2",
            None,
        ),
        (
            ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricFourLegLandmarkBase,
            ori_domain::BeginnerTargetPartKindV1::Leg,
            4,
            "asymmetric-four-leg.ori2",
            None,
        ),
        (
            ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricInsectLandmarkBase,
            ori_domain::BeginnerTargetPartKindV1::Tail,
            1,
            "asymmetric-insect.ori2",
            Some(10),
        ),
        (
            ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricFishLandmarkBase,
            ori_domain::BeginnerTargetPartKindV1::Fin,
            2,
            "asymmetric-fish.ori2",
            Some(4),
        ),
    ] {
        let insect_landmarks = semantic_binding_count == Some(10);
        let fish_landmarks = semantic_binding_count == Some(4);
        let mut profile = ori_domain::BeginnerDesignProfileV1::default();
        profile.generation_constraints.target_category =
            Some(ori_domain::BeginnerTargetCategoryV1::Animal);
        profile.generation_constraints.target_category = Some(if insect_landmarks {
            ori_domain::BeginnerTargetCategoryV1::Insect
        } else {
            ori_domain::BeginnerTargetCategoryV1::Animal
        });
        profile.generation_constraints.target_parts = (if insect_landmarks {
            vec![
                (ori_domain::BeginnerTargetPartKindV1::Head, 1),
                (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
                (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
                (ori_domain::BeginnerTargetPartKindV1::Wing, 2),
                (ori_domain::BeginnerTargetPartKindV1::Leg, 6),
            ]
        } else if fish_landmarks {
            vec![
                (ori_domain::BeginnerTargetPartKindV1::Head, 1),
                (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
                (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
                (ori_domain::BeginnerTargetPartKindV1::Fin, 2),
            ]
        } else {
            vec![
                (ori_domain::BeginnerTargetPartKindV1::Head, 1),
                (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
                (target_kind, target_count),
            ]
        })
        .into_iter()
        .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
        .collect();
        configure_symmetric_profile(
            &mut profile,
            ori_domain::BeginnerSymmetricParameterEstimateV1 {
                protrusion_count: 2,
                scale_percent: 27,
                spacing_percent: 50,
            },
            27,
            50,
        );
        profile.generation_constraints.skeleton_segments.truncate(
            if target_count == 4 || insect_landmarks {
                3
            } else {
                2
            },
        );
        profile.generation_constraints.skeleton_segments[0]
            .start
            .x_tenths_mm = -10;
        profile.generation_constraints.skeleton_segments[0]
            .start
            .y_tenths_mm = 0;
        profile.generation_constraints.skeleton_segments[0]
            .end
            .x_tenths_mm = 0;
        profile.generation_constraints.skeleton_segments[0]
            .end
            .y_tenths_mm = 10;
        profile.generation_constraints.skeleton_segments[1]
            .start
            .x_tenths_mm = 10;
        profile.generation_constraints.skeleton_segments[1]
            .start
            .y_tenths_mm = 0;
        profile.generation_constraints.skeleton_segments[1]
            .end
            .x_tenths_mm = 0;
        profile.generation_constraints.skeleton_segments[1]
            .end
            .y_tenths_mm = 10;
        let mut left = profile.generation_constraints.protrusions[0].clone();
        left.count = 1;
        left.length_tenths_mm = 4;
        left.thickness_tenths_mm = 2;
        left.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::None;
        left.position_tenths_mm = [-4, 0, 0];
        left.direction_milli = [-1_000, 200, 0];
        let mut right = left.clone();
        right.id = 2;
        right.position_tenths_mm = [5, 1, 0];
        right.direction_milli = [1_000, -100, 0];
        profile.generation_constraints.protrusions = if insect_landmarks {
            right.count = 2;
            right.symmetry = ori_domain::BeginnerProtrusionSymmetryV1::Bilateral;
            let mut targets = vec![left.clone(), right];
            let leg_positions: [(i16, i16); 6] =
                [(-5, 4), (5, 4), (-6, 0), (6, 0), (-5, -4), (5, -4)];
            for (offset, (x, y)) in leg_positions.into_iter().enumerate() {
                let mut leg = left.clone();
                leg.id = u16::try_from(offset + 3).unwrap();
                leg.position_tenths_mm = [i32::from(x), i32::from(y), 0];
                leg.direction_milli = [x.signum() * 1_000, y * 50, 0];
                targets.push(leg);
            }
            targets
        } else if fish_landmarks {
            let mut tail = left.clone();
            tail.id = 3;
            tail.position_tenths_mm = [0, -5, 0];
            tail.direction_milli = [100, -1_000, 0];
            vec![left, right, tail]
        } else if target_count == 4 {
            let mut rear_left = left.clone();
            rear_left.id = 3;
            rear_left.position_tenths_mm = [-5, -4, 0];
            rear_left.direction_milli = [-900, -300, 0];
            let mut rear_right = right.clone();
            rear_right.id = 4;
            rear_right.position_tenths_mm = [4, -5, 0];
            rear_right.direction_milli = [900, -200, 0];
            vec![left, right, rear_left, rear_right]
        } else {
            vec![left, right]
        };

        let half_height = 86.602_540_378_443_86;
        let mut project = ProjectState::new(CreasePattern::empty());
        let geometry_namespace = ProjectId::schema_namespace([
            0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x04, 0x97,
        ]);
        let boundary_positions = [
            Point2::new(100.0, 0.0),
            Point2::new(-50.0, half_height),
            Point2::new(-50.0, -half_height),
            Point2::new(50.0, -half_height),
        ];
        let vertices = boundary_positions
            .into_iter()
            .enumerate()
            .map(|(index, position)| Vertex {
                id: VertexId::derive_v5(geometry_namespace, format!("vertex-{index}").as_bytes()),
                position,
            })
            .collect::<Vec<_>>();
        let edges = (0..vertices.len())
            .map(|index| Edge {
                id: EdgeId::derive_v5(geometry_namespace, format!("boundary-{index}").as_bytes()),
                start: vertices[index].id,
                end: vertices[(index + 1) % vertices.len()].id,
                kind: EdgeKind::Boundary,
            })
            .collect();
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
            thickness_mm: 0.0,
            ..Paper::default()
        };
        project.editor = EditorState::with_paper(CreasePattern { vertices, edges }, paper);
        project.saved_document = Some(project.document());
        let project_id = project.project_id;
        let instance_id = project.instance_id;
        let revision = project.editor.revision();
        let saved = execute_command(
            &mut project,
            project_id,
            revision,
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(profile.clone()),
            },
        )
        .unwrap();
        let plan = ori_domain::generate_beginner_plans_v1(
            project_id,
            project.editor.pattern(),
            &project.editor.paper().boundary_vertices,
            &profile.generation_constraints,
        )
        .unwrap()
        .into_iter()
        .find(|plan| plan.kind == plan_kind)
        .unwrap();
        let candidate_edge = plan.crease_pattern.edges[0].id;
        assert_eq!(
            plan.crease_pattern
                .edges
                .iter()
                .filter(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
                .count(),
            4,
            "native-positive landmark DTO must remain a four-hinge tree candidate"
        );
        let preview = assess_beginner_generated_plan_with_deadline(
            project_id,
            project.editor.paper(),
            project.editor.pattern(),
            &plan,
            None,
            std::time::Instant::now() + std::time::Duration::from_millis(750),
        );
        assert!(
            preview.apply_allowed,
            "preview rejected: {}",
            preview.reason
        );
        assert_eq!(preview.proof_scope, "sufficient");
        assert!(matches!(
            preview.reason,
            "native_fold_path_certified" | "global_flat_foldability_proven"
        ));
        let canonical_edge_ids = plan
            .crease_pattern
            .edges
            .iter()
            .map(|edge| edge.id)
            .collect::<Vec<_>>();
        for _ in 0..32 {
            let authority = ProjectId::new();
            let authority_plan = ori_domain::generate_beginner_plans_v1(
                authority,
                project.editor.pattern(),
                &project.editor.paper().boundary_vertices,
                &profile.generation_constraints,
            )
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.kind == plan_kind)
            .unwrap();
            assert_eq!(
                authority_plan
                    .crease_pattern
                    .edges
                    .iter()
                    .map(|edge| edge.id)
                    .collect::<Vec<_>>(),
                canonical_edge_ids
            );
            assert_eq!(
                authority_plan.crease_pattern, plan.crease_pattern,
                "schema-derived landmark geometry must not depend on the runtime project authority"
            );
        }
        let state = AppState::new(project);
        let before = {
            let project = lock_project(&state).unwrap();
            project_state_signature(&project)
        };
        let mut tampered = profile.clone();
        tampered.generation_constraints.protrusions[0].priority += 1;
        for (foreign_instance, foreign_project, stale_revision) in [
            (ProjectId::new(), project_id, saved.revision),
            (instance_id, ProjectId::new(), saved.revision),
            (instance_id, project_id, saved.revision.saturating_sub(1)),
        ] {
            assert!(
                apply_beginner_generated_plan_document(
                    &state,
                    foreign_instance,
                    foreign_project,
                    stale_revision,
                    profile.clone(),
                    plan_kind,
                    candidate_edge,
                )
                .is_err()
            );
            assert_eq!(
                {
                    let project = lock_project(&state).unwrap();
                    project_state_signature(&project)
                },
                before
            );
        }
        assert!(
            apply_beginner_generated_plan_document(
                &state,
                instance_id,
                project_id,
                saved.revision,
                tampered,
                plan_kind,
                candidate_edge,
            )
            .is_err()
        );
        assert_eq!(
            {
                let project = lock_project(&state).unwrap();
                project_state_signature(&project)
            },
            before
        );

        let applied = apply_beginner_generated_plan_document(
            &state,
            instance_id,
            project_id,
            saved.revision,
            profile.clone(),
            plan_kind,
            candidate_edge,
        )
        .unwrap();
        let after_apply = {
            let project = lock_project(&state).unwrap();
            project_state_signature(&project)
        };
        for (rejected_instance, rejected_project, rejected_revision, rejected_edge) in [
            (instance_id, project_id, saved.revision, candidate_edge),
            (
                ProjectId::new(),
                project_id,
                applied.revision,
                candidate_edge,
            ),
            (
                instance_id,
                ProjectId::new(),
                applied.revision,
                candidate_edge,
            ),
            (instance_id, project_id, applied.revision, EdgeId::new()),
        ] {
            assert!(
                apply_beginner_generated_plan_document(
                    &state,
                    rejected_instance,
                    rejected_project,
                    rejected_revision,
                    profile.clone(),
                    plan_kind,
                    rejected_edge,
                )
                .is_err()
            );
            assert_eq!(
                {
                    let project = lock_project(&state).unwrap();
                    project_state_signature(&project)
                },
                after_apply
            );
        }
        let mut project = lock_project(&state).unwrap();
        let provenance = project
            .editor
            .beginner_design_profile()
            .generation_provenance
            .as_ref()
            .unwrap();
        assert!(provenance.fold_path_certificate_sha256.is_some());
        if let Some(expected_count) = semantic_binding_count {
            let semantic = provenance
                .semantic_landmark_provenance
                .as_ref()
                .expect("asymmetric semantic provenance");
            assert_eq!(semantic.ordered_bindings.len(), expected_count);
            assert_eq!(semantic.ordered_bindings[0].role, "head");
            assert_eq!(
                semantic.ordered_bindings.last().unwrap().role,
                if fish_landmarks {
                    "fin_right"
                } else {
                    "leg_rear_right"
                }
            );
            assert!(ori_domain::validate_beginner_generation_provenance_v1(
                provenance
            ));
        }
        let undone = execute_undo(&mut project, project_id, applied.revision).unwrap();
        assert!(
            project
                .editor
                .beginner_design_profile()
                .generation_provenance
                .is_none()
        );
        execute_redo(&mut project, project_id, undone.revision).unwrap();
        let document = project.document();
        let bytes = write_project_ori2(&document).unwrap();
        let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default()).unwrap();
        let reopened = ProjectState::from_valid_document(restored, PathBuf::from(archive_name));
        assert_eq!(reopened.document(), document);
        assert!(
            reopened
                .editor
                .beginner_design_profile()
                .generation_provenance
                .as_ref()
                .and_then(|value| value.fold_path_certificate_sha256)
                .is_some()
        );
        let instruction = reopened
            .editor
            .instruction_timeline()
            .steps
            .last()
            .expect("native-positive candidate instruction");
        let certificate = reopened
            .editor
            .beginner_design_profile()
            .generation_provenance
            .as_ref()
            .and_then(|value| value.fold_path_certificate_sha256)
            .expect("archived native fold-path certificate");
        let certificate_hex = certificate
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert!(instruction.caution.contains(&certificate_hex));
        if let Some(expected_count) = semantic_binding_count {
            assert_eq!(
                reopened
                    .editor
                    .beginner_design_profile()
                    .generation_provenance
                    .as_ref()
                    .and_then(|value| value.semantic_landmark_provenance.as_ref())
                    .map(|semantic| semantic.ordered_bindings.len()),
                Some(expected_count)
            );
        }
    }
}

#[test]
fn beginner_certifier_matches_positive_five_and_eight_hinge_tree_fixtures() {
    let fixtures: [&[(f64, f64)]; 2] = [
        &[
            (0., 0.),
            (300., 0.),
            (520., 90.),
            (680., 280.),
            (650., 500.),
            (450., 680.),
            (180., 700.),
            (0., 340.),
        ],
        &[
            (0., 0.),
            (300., 0.),
            (540., 60.),
            (730., 190.),
            (840., 380.),
            (850., 570.),
            (760., 750.),
            (590., 880.),
            (370., 930.),
            (150., 850.),
            (0., 430.),
        ],
    ];
    for (hinges, points) in [5_usize, 8].into_iter().zip(fixtures) {
        let ns = ProjectId::new();
        let vertices = points
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| Vertex {
                id: VertexId::derive_v5(ns, format!("v{i}").as_bytes()),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = (0..vertices.len())
            .map(|i| Edge {
                id: EdgeId::derive_v5(ns, format!("b{i}").as_bytes()),
                start: vertices[i].id,
                end: vertices[(i + 1) % vertices.len()].id,
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        let creases = (2..=hinges + 1)
            .enumerate()
            .map(|(i, end)| Edge {
                id: EdgeId::derive_v5(ns, format!("h{i}").as_bytes()),
                start: vertices[0].id,
                end: vertices[end].id,
                kind: if i.is_multiple_of(2) {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            })
            .collect::<Vec<_>>();
        let paper = Paper {
            boundary_vertices: vertices.iter().map(|v| v.id).collect(),
            ..Paper::default()
        };
        let current = CreasePattern {
            vertices: vertices.clone(),
            edges: boundary,
        };
        let plan = ori_domain::BeginnerGeneratedPlanV1 {
            schema_version: 1,
            kind: ori_domain::BeginnerGeneratedPlanKindV1::AsymmetricFourLegLandmarkBase,
            crease_pattern: CreasePattern {
                vertices,
                edges: creases,
            },
            instruction_codes: vec![format!("tree-{hinges}")],
            target_parts: Vec::new(),
            skeleton_segments: Vec::new(),
            target_asset: None,
            semantic_landmark_provenance: None,
        };
        let assessment = assess_beginner_generated_plan_with_deadline(
            ns,
            &paper,
            &current,
            &plan,
            None,
            std::time::Instant::now() + std::time::Duration::from_millis(750),
        );
        assert!(assessment.apply_allowed, "{hinges}: {}", assessment.reason);
        assert_eq!(
            (assessment.proof_scope, assessment.reason),
            ("sufficient", "native_fold_path_certified")
        );
        let canonical_assessment = serde_json::to_vec(&assessment).unwrap();
        for repetition in 0..8 {
            let repeated = assess_beginner_generated_plan_with_deadline(
                ns,
                &paper,
                &current,
                &plan,
                None,
                std::time::Instant::now() + std::time::Duration::from_millis(750),
            );
            assert_eq!(
                serde_json::to_vec(&repeated).unwrap(),
                canonical_assessment,
                "{hinges}-hinge assessment repetition {repetition} must be deterministic"
            );
        }
        let mut candidate = current.clone();
        candidate
            .edges
            .extend(plan.crease_pattern.edges.iter().cloned());
        let candidate_editor = EditorState::with_paper(candidate.clone(), paper.clone());
        let candidate_fingerprint = candidate_editor.fold_model_fingerprint_v1();
        let topology = candidate_editor.topology_analysis_input(ns).analyze();
        let certificate = certify_beginner_fold_path_v1(
            &plan,
            &paper,
            &candidate,
            topology
                .simulation_snapshot()
                .expect("positive tree topology"),
        )
        .expect("positive tree certificate");
        let authority: [u8; 32] =
            sha2::Sha256::digest(serde_json::to_vec(&candidate).unwrap()).into();
        let certificate_hex = certificate
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let mut project = ProjectState::new_with_paper(current, paper.clone());
        let mut profile = project.editor.beginner_design_profile().clone();
        profile.generation_provenance = Some(ori_domain::BeginnerGenerationProvenanceV1 {
            schema_version: 1,
            topology_authority_sha256: authority,
            fold_path_certificate_sha256: Some(certificate),
            confidence_score: 100,
            confidence_reasons: vec!["bounded_native_fold_path_v2".to_owned()],
            explicit_override: false,
            source_asset_fingerprint: format!("native-positive-tree-{hinges}"),
            semantic_landmark_provenance: None,
            generic_tree: Some(ori_domain::BeginnerGenericTreeProvenanceV1 {
                schema_version: 1,
                target_category: None,
                source: ori_domain::BeginnerGenericTreeSourceV1::ManualSkeleton,
                asset_content_sha256: None,
                tree_topology_sha256: authority,
                normalized_length_ratios: vec![1_000_000; hinges],
                orientation: ori_domain::BeginnerGenericTreeOrientationV1::Horizontal,
                generator_version: 1,
                authorizes_apply: false,
                instruction_proposal: None,
            }),
            reference_consensus_summary: None,
            reference_consensus: None,
        });
        let mut timeline = project.editor.instruction_timeline().clone();
        timeline.steps.push(InstructionStep {
            id: InstructionStepId::new(),
            title: format!("{hinges}-hinge generic tree"),
            description: "Apply the native-proven generic tree candidate.".to_owned(),
            caution: format!("Native fold-path certificate SHA-256: {certificate_hex}."),
            duration_ms: 2_000,
            visual: InstructionVisual::default(),
            pose: InstructionPose {
                model: InstructionPoseModel::DeclarativeOnlyV1,
                source_model_fingerprint: candidate_fingerprint.clone(),
                fixed_face: None,
                hinge_angles: Vec::new(),
            },
        });
        let project_id = project.project_id;
        let revision = project.editor.revision();
        let layers = project.editor.project_layers().clone();
        let applied = execute_command(
            &mut project,
            project_id,
            revision,
            Command::ApplyStackedFoldDocument {
                pattern: candidate,
                paper,
                instruction_timeline: timeline,
                project_layers: layers,
                beginner_design_profile: Box::new(profile),
            },
        )
        .expect("apply native-positive generic tree");
        assert_eq!(
            project.editor.fold_model_fingerprint_v1(),
            candidate_fingerprint
        );
        assert_eq!(
            project
                .editor
                .instruction_timeline()
                .steps
                .last()
                .expect("applied generic tree instruction")
                .pose
                .source_model_fingerprint,
            candidate_fingerprint
        );
        assert!(validate_document_instruction_poses(&project.document()).is_ok());
        let undone = execute_undo(&mut project, project_id, applied.revision).unwrap();
        assert!(
            project
                .editor
                .beginner_design_profile()
                .generation_provenance
                .is_none()
        );
        execute_redo(&mut project, project_id, undone.revision).unwrap();
        assert_eq!(
            project.editor.fold_model_fingerprint_v1(),
            candidate_fingerprint
        );
        assert!(validate_document_instruction_poses(&project.document()).is_ok());
        let document = project.document();
        let bytes = write_project_ori2(&document).unwrap();
        let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default()).unwrap();
        assert_eq!(restored, document);
        let restored_certificate = restored
            .beginner_design_profile
            .generation_provenance
            .as_ref()
            .and_then(|value| value.fold_path_certificate_sha256)
            .unwrap();
        assert_eq!(restored_certificate, certificate);
        let restored_topology =
            EditorState::with_paper(restored.crease_pattern.clone(), restored.paper.clone())
                .topology_analysis_input(ns)
                .analyze();
        let recertified = certify_beginner_fold_path_v1(
            &plan,
            &restored.paper,
            &restored.crease_pattern,
            restored_topology
                .simulation_snapshot()
                .expect("restored positive tree topology"),
        )
        .expect("recertify restored positive tree");
        assert_eq!(recertified, restored_certificate);
        let mut assignment_tampered = restored.crease_pattern.clone();
        let crease = assignment_tampered
            .edges
            .iter_mut()
            .find(|edge| matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley))
            .expect("restored generic tree crease");
        crease.kind = match crease.kind {
            EdgeKind::Mountain => EdgeKind::Valley,
            EdgeKind::Valley => EdgeKind::Mountain,
            _ => unreachable!("selected only an assigned crease"),
        };
        let tampered_topology =
            EditorState::with_paper(assignment_tampered.clone(), restored.paper.clone())
                .topology_analysis_input(ns)
                .analyze();
        assert_ne!(
            certify_beginner_fold_path_v1(
                &plan,
                &restored.paper,
                &assignment_tampered,
                tampered_topology
                    .simulation_snapshot()
                    .expect("assignment-tampered tree topology"),
            ),
            Some(restored_certificate),
            "the fold-path certificate must bind the mountain/valley assignment"
        );
        let mut geometry_tampered = restored.crease_pattern.clone();
        geometry_tampered
            .vertices
            .last_mut()
            .expect("restored generic tree vertex")
            .position
            .x += 1.0;
        let geometry_topology =
            EditorState::with_paper(geometry_tampered.clone(), restored.paper.clone())
                .topology_analysis_input(ns)
                .analyze();
        assert_ne!(
            certify_beginner_fold_path_v1(
                &plan,
                &restored.paper,
                &geometry_tampered,
                geometry_topology
                    .simulation_snapshot()
                    .expect("geometry-tampered tree topology"),
            ),
            Some(restored_certificate),
            "the 3D fold-path certificate must bind the face geometry"
        );
        assert!(
            restored
                .instruction_timeline
                .steps
                .last()
                .unwrap()
                .caution
                .contains(&certificate_hex)
        );
        let archive = project.project_archive().expect("generic tree archive");
        let archive_bytes = write_project_archive_ori2(&archive).expect("write generic tree ORI2");
        let archive_restored =
            read_project_archive_ori2(&archive_bytes).expect("read generic tree ORI2");
        assert_eq!(archive_restored, archive);
        assert_eq!(
            write_project_archive_ori2(&archive_restored)
                .expect("canonically resave generic tree ORI2"),
            archive_bytes
        );
        assert!(
            read_project_archive_ori2(&tamper_ori2_project_certificate(&archive_bytes, false,))
                .is_err(),
            "an authenticated ORI2 must reject certificate provenance tampering"
        );
        assert!(
            read_project_archive_ori2(&tamper_ori2_project_certificate(&archive_bytes, true,))
                .is_err(),
            "an ORI2 must reject reauthenticated project provenance that diverges from history"
        );
        let folder = write_project_folder_v1(&archive).expect("write generic tree folder");
        let mut tampered_entries = folder.entries().to_vec();
        let (project_size, project_sha256) = {
            let project_entry = tampered_entries
                .iter_mut()
                .find(|entry| entry.path == ori_formats::PROJECT_FOLDER_PROJECT_PATH)
                .expect("generic tree project entry");
            let mut tampered_json: serde_json::Value =
                serde_json::from_slice(&project_entry.bytes).unwrap();
            let certificate_byte = tampered_json
                .pointer_mut(
                    "/beginner_design_profile/generation_provenance/fold_path_certificate_sha256/0",
                )
                .expect("generic tree certificate byte");
            *certificate_byte =
                serde_json::json!(certificate_byte.as_u64().unwrap_or_default() ^ 1);
            project_entry.bytes = serde_json::to_vec(&tampered_json).unwrap();
            let sha256 = sha2::Sha256::digest(&project_entry.bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            (project_entry.bytes.len() as u64, sha256)
        };
        assert!(
            read_project_folder_v1(&tampered_entries).is_err(),
            "an authenticated folder must reject certificate provenance tampering"
        );
        let manifest_entry = tampered_entries
            .iter_mut()
            .find(|entry| entry.path == ori_formats::PROJECT_FOLDER_MANIFEST_PATH)
            .expect("generic tree manifest entry");
        let mut manifest: ori_formats::ProjectFolderManifestV1 =
            serde_json::from_slice(&manifest_entry.bytes).unwrap();
        let descriptor = manifest
            .entries
            .iter_mut()
            .find(|entry| entry.path == ori_formats::PROJECT_FOLDER_PROJECT_PATH)
            .expect("generic tree project descriptor");
        descriptor.uncompressed_size = project_size;
        descriptor.sha256 = project_sha256;
        manifest_entry.bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        assert!(
            read_project_folder_v1(&tampered_entries).is_err(),
            "a folder must reject reauthenticated project provenance that diverges from history"
        );
        let folder_restored = read_project_folder_v1(folder.entries())
            .expect("read generic tree folder")
            .into_archive();
        assert_eq!(folder_restored, archive);
        assert_eq!(
            write_project_folder_v1(&folder_restored)
                .expect("canonically resave generic tree folder")
                .entries(),
            folder.entries()
        );
        let folder_provenance = folder_restored
            .document
            .beginner_design_profile
            .generation_provenance
            .as_ref()
            .expect("folder generic tree provenance");
        assert_eq!(
            folder_provenance.fold_path_certificate_sha256,
            Some(certificate)
        );
        let folder_topology = EditorState::with_paper(
            folder_restored.document.crease_pattern.clone(),
            folder_restored.document.paper.clone(),
        )
        .topology_analysis_input(ns)
        .analyze();
        assert_eq!(
            certify_beginner_fold_path_v1(
                &plan,
                &folder_restored.document.paper,
                &folder_restored.document.crease_pattern,
                folder_topology
                    .simulation_snapshot()
                    .expect("folder-restored positive tree topology"),
            )
            .expect("recertify folder-restored positive tree"),
            certificate
        );
        assert!(folder_provenance.generic_tree.is_some());
        assert!(
            folder_restored
                .document
                .instruction_timeline
                .steps
                .last()
                .unwrap()
                .caution
                .contains(&certificate_hex)
        );
        let mut recovered = ProjectState::from_recovery_project_archive(archive.clone())
            .expect("recover generic tree archive");
        let recovered_document = recovered.document();
        assert_eq!(
            recovered_document.crease_pattern,
            archive.document.crease_pattern
        );
        assert_eq!(recovered_document.paper, archive.document.paper);
        assert_eq!(
            recovered_document.instruction_timeline,
            archive.document.instruction_timeline
        );
        assert_eq!(
            recovered_document.beginner_design_profile,
            archive.document.beginner_design_profile
        );
        let recovered_topology = EditorState::with_paper(
            recovered_document.crease_pattern.clone(),
            recovered_document.paper.clone(),
        )
        .topology_analysis_input(ns)
        .analyze();
        assert_eq!(
            certify_beginner_fold_path_v1(
                &plan,
                &recovered_document.paper,
                &recovered_document.crease_pattern,
                recovered_topology
                    .simulation_snapshot()
                    .expect("recovered positive tree topology"),
            )
            .expect("recertify recovered positive tree"),
            certificate
        );
        let recovered_provenance = recovered
            .editor
            .beginner_design_profile()
            .generation_provenance
            .as_ref()
            .expect("recovered generic tree provenance");
        assert_eq!(
            recovered_provenance.fold_path_certificate_sha256,
            Some(certificate)
        );
        assert!(recovered_provenance.generic_tree.is_some());
        assert!(
            recovered
                .editor
                .instruction_timeline()
                .steps
                .last()
                .unwrap()
                .caution
                .contains(&certificate_hex)
        );
        assert!(recovered.editor.can_undo());
        let recovered_revision = recovered.editor.revision();
        let recovered_undo = execute_undo(&mut recovered, project_id, recovered_revision)
            .expect("undo recovered generic tree");
        assert!(
            recovered
                .editor
                .beginner_design_profile()
                .generation_provenance
                .is_none()
        );
        execute_redo(&mut recovered, project_id, recovered_undo.revision)
            .expect("redo recovered generic tree");
        assert_eq!(
            recovered
                .editor
                .beginner_design_profile()
                .generation_provenance
                .as_ref()
                .and_then(|value| value.fold_path_certificate_sha256),
            Some(certificate)
        );
    }
}

#[test]
fn complete_animal_grid_apply_replay_undo_redo_and_archive_round_trip() {
    let _serial = serial_beginner_grid_test();
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    profile.generation_constraints.target_parts = vec![
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Horn, 1),
        (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
        (ori_domain::BeginnerTargetPartKindV1::Ear, 2),
        (ori_domain::BeginnerTargetPartKindV1::Leg, 4),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    configure_symmetric_profile(
        &mut profile,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 8,
            scale_percent: 25,
            spacing_percent: 50,
        },
        25,
        50,
    );
    assert!(ori_domain::animal_complete_bindings_v1(&profile.generation_constraints).is_some());

    let point = ori_domain::beginner_parameter_grid_v1()[13];
    let apply_profile = profile.clone();
    for target in &mut profile.generation_constraints.protrusions {
        target.length_tenths_mm = 270 + u32::from(target.id) * 10;
        target.thickness_tenths_mm = 50 + target.id;
        target.direction_milli[0] = -target.direction_milli[0];
        target.direction_milli[1] = -target.direction_milli[1];
    }
    profile.generation_constraints.protrusions.reverse();
    let temporary = temporary_symmetric_profile_for_grid(&profile, point).unwrap();
    assert_eq!(
        temporary
            .generation_constraints
            .protrusions
            .iter()
            .map(|target| target.id)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4]
    );
    for target in &temporary.generation_constraints.protrusions {
        assert_eq!(
            target.length_tenths_mm,
            ((270 + u32::from(target.id) * 10) * u32::from(point.scale_percent) / 27).max(1)
        );
        assert_eq!(
            target.thickness_tenths_mm,
            ((50 + target.id) * u16::from(point.spacing_percent) / 50).max(1)
        );
    }
    let mut project = initial_project_state();
    let plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &apply_profile,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|plan| plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteAnimalBase)
    .unwrap();
    let project_id = project.project_id;
    let instance_id = project.instance_id;
    let revision = project.editor.revision();
    let saved_profile = execute_command(
        &mut project,
        project_id,
        revision,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(apply_profile),
        },
    )
    .unwrap();
    assert!(
        apply_grid_plan_document(
            &mut project,
            instance_id,
            project_id,
            revision,
            plan.clone(),
        )
        .is_err()
    );
    let applied = apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        saved_profile.revision,
        plan.clone(),
    )
    .unwrap();
    assert!(
        apply_grid_plan_document(
            &mut project,
            instance_id,
            project_id,
            saved_profile.revision,
            plan,
        )
        .is_err()
    );
    let undone = execute_undo(&mut project, project_id, applied.revision).unwrap();
    let _redone = execute_redo(&mut project, project_id, undone.revision).unwrap();
    let saved = project.document();
    let bytes = write_project_ori2(&saved).unwrap();
    let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default()).unwrap();
    let reopened =
        ProjectState::from_valid_document(restored, PathBuf::from("complete-animal.ori2"));
    assert_eq!(reopened.document(), saved);
    assert!(
        ori_domain::animal_complete_bindings_v1(
            &reopened
                .editor
                .beginner_design_profile()
                .generation_constraints
        )
        .is_some()
    );
    assert!(!reopened.editor.can_undo());
    assert!(!reopened.editor.can_redo());
}

#[test]
fn complete_winged_animal_grid_apply_and_archive_round_trip() {
    let _serial = serial_beginner_grid_test();
    let mut profile = ori_domain::BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_category =
        Some(ori_domain::BeginnerTargetCategoryV1::Animal);
    profile.generation_constraints.target_parts = vec![
        (ori_domain::BeginnerTargetPartKindV1::Head, 1),
        (ori_domain::BeginnerTargetPartKindV1::Torso, 1),
        (ori_domain::BeginnerTargetPartKindV1::Horn, 1),
        (ori_domain::BeginnerTargetPartKindV1::Tail, 1),
        (ori_domain::BeginnerTargetPartKindV1::Ear, 2),
        (ori_domain::BeginnerTargetPartKindV1::Leg, 4),
        (ori_domain::BeginnerTargetPartKindV1::Wing, 2),
    ]
    .into_iter()
    .map(|(kind, count)| ori_domain::BeginnerTargetPartRecordV1 { kind, count })
    .collect();
    configure_symmetric_profile(
        &mut profile,
        ori_domain::BeginnerSymmetricParameterEstimateV1 {
            protrusion_count: 10,
            scale_percent: 25,
            spacing_percent: 50,
        },
        25,
        50,
    );
    let binding = ori_domain::animal_complete_winged_bindings_v1(&profile.generation_constraints)
        .expect("strict five-binding winged animal");
    assert_eq!(binding.wing_pair_protrusion_id, 5);
    let point = ori_domain::beginner_parameter_grid_v1()[13];
    let mut project = initial_project_state();
    let plan = grid_template_plan(
        project.project_id,
        project.editor.pattern(),
        &project.editor.paper().boundary_vertices,
        &profile,
        point,
    )
    .unwrap()
    .into_iter()
    .find(|plan| {
        plan.kind == ori_domain::BeginnerGeneratedPlanKindV1::CompositeCompleteWingedAnimalBase
    })
    .expect("winged animal grid plan");
    assert_eq!(plan.crease_pattern.vertices.len(), 15);
    assert_eq!(plan.crease_pattern.edges.len(), 14);
    let project_id = project.project_id;
    let instance_id = project.instance_id;
    let cancel_generation = ProjectId::new();
    let cancel_work = Arc::new(BeginnerGridWork::default());
    beginner_grid_work()
        .lock()
        .unwrap()
        .insert(cancel_generation, Arc::clone(&cancel_work));
    cancel_beginner_parameter_grid(cancel_generation).unwrap();
    assert!(cancel_work.cancelled.load(Ordering::Acquire));
    beginner_grid_work().lock().unwrap().clear();
    let revision = project.editor.revision();
    let saved_profile = execute_command(
        &mut project,
        project_id,
        revision,
        Command::UpdateBeginnerDesignProfile {
            profile: Box::new(profile),
        },
    )
    .unwrap();
    let applied = apply_grid_plan_document(
        &mut project,
        instance_id,
        project_id,
        saved_profile.revision,
        plan.clone(),
    )
    .unwrap();
    assert!(
        apply_grid_plan_document(
            &mut project,
            instance_id,
            project_id,
            saved_profile.revision,
            plan,
        )
        .is_err()
    );
    let undone = execute_undo(&mut project, project_id, applied.revision).unwrap();
    execute_redo(&mut project, project_id, undone.revision).unwrap();
    let mut saved = project.document();
    saved.thumbnail_svg = None;
    let bytes = write_project_ori2(&saved).unwrap();
    let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default()).unwrap();
    let reopened = ProjectState::from_valid_document(restored, PathBuf::from("winged-animal.ori2"));
    assert_eq!(
        reopened.editor.beginner_design_profile(),
        &saved.beginner_design_profile
    );
    assert!(
        ori_domain::animal_complete_winged_bindings_v1(
            &reopened
                .editor
                .beginner_design_profile()
                .generation_constraints,
        )
        .is_some()
    );
}

#[test]
fn symmetry_transforms_are_exact_at_cardinal_angles() {
    assert_eq!(
        mirror_point_left_right(Point2::new(3.0, 4.0), 1.0),
        Point2::new(-1.0, 4.0)
    );
    let center = Point2::new(1.0, 2.0);
    let point = Point2::new(3.0, 4.0);
    for (angle, expected) in [
        (0.0, Point2::new(3.0, 4.0)),
        (90.0, Point2::new(-1.0, 4.0)),
        (180.0, Point2::new(-1.0, 0.0)),
        (270.0, Point2::new(3.0, 0.0)),
    ] {
        let (sin, cos) = symmetry_sin_cos(angle);
        assert_eq!(rotate_point_about(point, center, sin, cos), expected);
    }
}

fn execute_command(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    command: Command,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_command(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        command,
    )
}

fn execute_undo(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_undo(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )
}

fn execute_redo(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_redo(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
    )
}

fn execute_edge_split(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_edge_split(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        edge,
        fraction,
    )
}

fn execute_edge_intersection_connection(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<EdgeIntersectionResponse, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_edge_intersection_connection(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        first_edge,
        second_edge,
    )
}

fn execute_intersection_cluster_connection(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    targets: Vec<IntersectionClusterTargetRequest>,
    junction_vertex_id: Option<VertexId>,
) -> Result<EdgeIntersectionResponse, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_intersection_cluster_connection(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        targets,
        junction_vertex_id,
    )
}

fn execute_t_junction_connection(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    first_edge: EdgeId,
    second_edge: EdgeId,
) -> Result<TJunctionResponse, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_t_junction_connection(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        first_edge,
        second_edge,
    )
}

fn execute_boundary_split(
    project: &mut ProjectState,
    expected_project_id: ProjectId,
    expected_revision: u64,
    edge: EdgeId,
    fraction: f64,
) -> Result<ProjectSnapshot, String> {
    let expected_project_instance_id = project.instance_id;
    super::execute_boundary_split(
        project,
        expected_project_instance_id,
        expected_project_id,
        expected_revision,
        edge,
        fraction,
    )
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must follow the Unix epoch")
            .as_nanos();
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, AtomicOrdering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "origami2-native-file-tests-{}-{timestamp}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated native-file test directory");
        Self { path }
    }

    #[cfg(target_os = "windows")]
    fn new_relative() -> Self {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, AtomicOrdering::Relaxed);
        let path = PathBuf::from(format!(
            ".origami2-relative-native-file-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated relative native-file test directory");
        Self { path }
    }

    fn join(&self, name: impl AsRef<Path>) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn new_project_parameters() -> NewProjectParameters {
    NewProjectParameters {
        name: "  Test sheet  ".to_owned(),
        width_expression: "210".to_owned(),
        height_expression: "297".to_owned(),
        width_mm: 210.0,
        height_mm: 297.0,
        thickness_mm: 0.2,
        cutting_allowed: true,
        front_color: RgbaColor {
            red: 10,
            green: 20,
            blue: 30,
            alpha: 240,
        },
        back_color: RgbaColor {
            red: 220,
            green: 210,
            blue: 200,
            alpha: 230,
        },
    }
}

fn cellular_multi_fold_project_state() -> ProjectState {
    let positions = [
        Point2::new(0.0, 0.0),
        Point2::new(2.0, 0.0),
        Point2::new(6.0, 0.0),
        Point2::new(8.0, 0.0),
        Point2::new(8.0, 6.0),
        Point2::new(6.0, 6.0),
        Point2::new(2.0, 6.0),
        Point2::new(0.0, 6.0),
    ];
    let vertices = positions
        .into_iter()
        .map(|position| Vertex {
            id: VertexId::new(),
            position,
        })
        .collect::<Vec<_>>();
    let mut edges = (0..vertices.len())
        .map(|index| Edge {
            id: EdgeId::new(),
            start: vertices[index].id,
            end: vertices[(index + 1) % vertices.len()].id,
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend([
        Edge {
            id: EdgeId::new(),
            start: vertices[1].id,
            end: vertices[6].id,
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: EdgeId::new(),
            start: vertices[2].id,
            end: vertices[5].id,
            kind: EdgeKind::Valley,
        },
    ]);
    let paper = Paper {
        boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
        ..Paper::default()
    };
    ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper)
}

fn four_ray_square_project_state(
    fold_endpoint_indices: [usize; 4],
    assignments: [EdgeKind; 4],
) -> (ProjectState, VertexId) {
    let boundary_positions = [
        Point2::new(0.0, 0.0),
        Point2::new(10.0, 0.0),
        Point2::new(20.0, 0.0),
        Point2::new(20.0, 10.0),
        Point2::new(20.0, 20.0),
        Point2::new(10.0, 20.0),
        Point2::new(0.0, 20.0),
        Point2::new(0.0, 10.0),
    ];
    let mut vertices = boundary_positions
        .into_iter()
        .map(|position| Vertex {
            id: VertexId::new(),
            position,
        })
        .collect::<Vec<_>>();
    let center = Vertex {
        id: VertexId::new(),
        position: Point2::new(10.0, 10.0),
    };
    let center_id = center.id;
    vertices.push(center);

    let mut edges = (0..boundary_positions.len())
        .map(|index| Edge {
            id: EdgeId::new(),
            start: vertices[index].id,
            end: vertices[(index + 1) % boundary_positions.len()].id,
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend(
        fold_endpoint_indices
            .into_iter()
            .zip(assignments)
            .map(|(endpoint, kind)| Edge {
                id: EdgeId::new(),
                start: center_id,
                end: vertices[endpoint].id,
                kind,
            }),
    );
    let paper = Paper {
        boundary_vertices: vertices[..boundary_positions.len()]
            .iter()
            .map(|vertex| vertex.id)
            .collect(),
        ..Paper::default()
    };
    (
        ProjectState::new_with_paper(CreasePattern { vertices, edges }, paper),
        center_id,
    )
}

#[derive(Debug, PartialEq)]
struct ProjectStateSignature {
    instance_id: ProjectId,
    project_id: ProjectId,
    document: ProjectDocument,
    editor_debug: String,
    applied_pose_authority: applied_pose::CurrentAppliedPoseAuthoritySnapshot,
    current_path: Option<PathBuf>,
    saved_revision: Option<u64>,
    saved_document: Option<ProjectDocument>,
    revision: u64,
    can_undo: bool,
    can_redo: bool,
    is_dirty: bool,
}

fn project_state_signature(project: &ProjectState) -> ProjectStateSignature {
    ProjectStateSignature {
        instance_id: project.instance_id,
        project_id: project.project_id,
        document: project.document(),
        editor_debug: format!("{:?}", project.editor),
        applied_pose_authority: project
            .applied_pose_authority
            .test_snapshot()
            .expect("capture applied-pose authority"),
        current_path: project.current_path.clone(),
        saved_revision: project.saved_revision,
        saved_document: project.saved_document.clone(),
        revision: project.editor.revision(),
        can_undo: project.editor.can_undo(),
        can_redo: project.editor.can_redo(),
        is_dirty: project.is_dirty(),
    }
}

fn geometric_constraint_binding(state: &AppState) -> (ProjectId, ProjectId, u64) {
    let project = lock_project(state).expect("lock geometric-constraint project");
    (
        project.instance_id,
        project.project_id,
        project.editor.revision(),
    )
}

fn geometric_constraint_project_signature(state: &AppState) -> ProjectStateSignature {
    let project = lock_project(state).expect("lock geometric-constraint project");
    project_state_signature(&project)
}

fn run_default_geometric_constraint_analysis(
    state: &AppState,
    binding: (ProjectId, ProjectId, u64),
) -> Result<GeometricConstraintPreflightResponse, String> {
    tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
        state,
        binding.0,
        binding.1,
        binding.2,
        ProjectId::new(),
        |pattern, document, _runtime| {
            Ok(analyze_geometric_constraint_document(&pattern, &document))
        },
    ))
}

fn wait_for_geometric_constraint_worker_idle(state: &Arc<AppState>) {
    let observer_state = Arc::clone(state);
    let (idle_tx, idle_rx) = mpsc::sync_channel(0);
    let observer = thread::spawn(move || {
        while observer_state.geometric_constraint_worker_is_busy() {
            thread::yield_now();
        }
        idle_tx.send(()).expect("announce idle worker gate");
    });
    idle_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("geometric-constraint worker gate must become idle");
    observer
        .join()
        .expect("worker-gate observer must not panic");
}

#[test]
fn geometric_constraint_document_is_dirty_undoable_and_loadable() {
    let mut project = initial_project_state();
    let edge = project.editor.pattern().edges[0].id;
    let record = GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint: GeometricConstraintKindV1::Horizontal { edge },
    };
    let project_id = project.project_id;

    let added = execute_command(
        &mut project,
        project_id,
        0,
        Command::AddGeometricConstraint {
            record: record.clone(),
        },
    )
    .expect("add constraint through native project bridge");
    assert_eq!(
        added.geometric_constraints.constraints,
        vec![record.clone()]
    );
    assert!(added.is_dirty);
    assert_eq!(
        project.document().geometric_constraints.constraints,
        vec![record.clone()]
    );

    let undone = execute_undo(&mut project, project_id, 1).expect("undo constraint");
    assert!(undone.geometric_constraints.is_empty());
    assert!(!undone.is_dirty);
    let redone = execute_redo(&mut project, project_id, 2).expect("redo constraint");
    assert_eq!(
        redone.geometric_constraints.constraints,
        vec![record.clone()]
    );
    assert!(redone.is_dirty);

    let document = project.document();
    let loaded =
        ProjectState::from_valid_document(document.clone(), PathBuf::from("constraint.ori2"));
    assert_eq!(loaded.document(), document);
    assert_eq!(
        loaded.editor.geometric_constraints().constraints,
        vec![record]
    );
    assert!(!loaded.is_dirty());
    assert!(!loaded.editor.can_undo());
    assert!(!loaded.editor.can_redo());
}

#[test]
fn project_layers_are_snapshotted_dirty_tracked_saved_and_reopened_with_history() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let edge = project.editor.pattern().edges[0].id;
    let layer = LayerRecordV1 {
        id: ori_domain::LayerId::new(),
        name: "Details".to_owned(),
        content_kind: LayerContentKindV1::CreasePattern,
        visible: true,
        locked: false,
        opacity: 1.0,
    };

    let created = execute_command(
        &mut project,
        project_id,
        0,
        Command::CreateLayer {
            layer: layer.clone(),
            target_index: 1,
        },
    )
    .expect("create layer through native project bridge");
    assert_eq!(created.project_layers.layers[1], layer);
    assert!(created.project_layers.edge_assignments.is_empty());
    assert!(created.is_dirty);

    let assigned = execute_command(
        &mut project,
        project_id,
        1,
        Command::AssignEdgeToLayer {
            edge,
            layer: layer.id,
        },
    )
    .expect("assign edge through native project bridge");
    assert_eq!(assigned.project_layers.layer_for_edge(edge), layer.id);
    assert_eq!(project.document().layers, assigned.project_layers);
    assert!(project.is_dirty());

    let presented = execute_command(
        &mut project,
        project_id,
        2,
        Command::UpdateLayerPresentation {
            layer: layer.id,
            visible: false,
            locked: true,
            opacity: 0.25,
        },
    )
    .expect("update layer presentation through native project bridge");
    assert_eq!(project.document().layers, presented.project_layers);
    assert!(!presented.project_layers.layers[1].visible);
    assert!(presented.project_layers.layers[1].locked);
    assert_eq!(presented.project_layers.layers[1].opacity, 0.25);

    let document = project.document();
    let loaded_without_history =
        ProjectState::from_valid_document(document.clone(), PathBuf::from("layers.ori2"));
    assert_eq!(
        loaded_without_history.editor.project_layers(),
        &document.layers
    );
    assert!(!loaded_without_history.is_dirty());

    let directory = TestDirectory::new();
    let path = directory.join("layer-history.ori2");
    save_project_to_path(&mut project, path.clone()).expect("save layered archive");
    assert!(!project.is_dirty());

    let mut reopened = ProjectState::new(CreasePattern::empty());
    let replaced_instance_id = reopened.instance_id;
    let replaced_project_id = reopened.project_id;
    let loaded = load_project_file(path.clone()).expect("load layered archive");
    apply_loaded_project_file(
        &mut reopened,
        replaced_instance_id,
        replaced_project_id,
        0,
        loaded,
    )
    .expect("apply layered archive");
    assert_eq!(reopened.document(), document);
    assert_eq!(reopened.editor.project_layers(), &document.layers);
    assert_eq!(snapshot(&reopened).project_layers, document.layers);
    assert!(!reopened.is_dirty());

    reopened
        .editor
        .undo(0)
        .expect("undo reopened layer presentation");
    assert!(reopened.editor.project_layers().layers[1].visible);
    assert!(!reopened.editor.project_layers().layers[1].locked);
    assert_eq!(reopened.editor.project_layers().layers[1].opacity, 1.0);
    reopened.editor.undo(1).expect("undo reopened assignment");
    assert_eq!(
        reopened.editor.project_layers().layer_for_edge(edge),
        ori_domain::DEFAULT_PROJECT_LAYER_ID
    );
    assert!(reopened.is_dirty());
    reopened
        .editor
        .undo(2)
        .expect("undo reopened layer creation");
    assert_eq!(
        reopened.editor.project_layers(),
        &ProjectLayerDocumentV1::default()
    );
    reopened
        .editor
        .redo(3)
        .expect("redo reopened layer creation");
    reopened.editor.redo(4).expect("redo reopened assignment");
    reopened
        .editor
        .redo(5)
        .expect("redo reopened layer presentation");
    assert_eq!(reopened.document(), document);
    assert!(!reopened.is_dirty());
}

#[test]
fn project_layer_ipc_helpers_guard_binding_and_apply_every_supported_mutation() {
    let mut project = initial_project_state();
    let project_instance_id = project.instance_id;
    let project_id = project.project_id;
    let edge = project.editor.pattern().edges[0].id;
    let original_document = project.document();

    assert!(
        create_project_layer_in_project(
            &mut project,
            ProjectId::new(),
            project_id,
            0,
            "Foreign".to_owned(),
            LayerContentKindV1::CreasePattern,
        )
        .is_err()
    );
    assert_eq!(project.document(), original_document);
    assert_eq!(project.editor.revision(), 0);

    let created_crease = create_project_layer_in_project(
        &mut project,
        project_instance_id,
        project_id,
        0,
        "Details".to_owned(),
        LayerContentKindV1::CreasePattern,
    )
    .expect("create crease-pattern layer");
    let crease_layer = created_crease.project_layers.layers[1].id;
    assert_eq!(created_crease.revision, 1);

    let created_annotation = create_project_layer_in_project(
        &mut project,
        project_instance_id,
        project_id,
        1,
        "Notes".to_owned(),
        LayerContentKindV1::Annotation,
    )
    .expect("create empty annotation layer");
    let annotation_layer = created_annotation.project_layers.layers[2].id;
    assert_eq!(
        created_annotation.project_layers.layers[2].content_kind,
        LayerContentKindV1::Annotation
    );

    let renamed = rename_project_layer_in_project(
        &mut project,
        project_instance_id,
        project_id,
        2,
        crease_layer,
        "Primary folds".to_owned(),
    )
    .expect("rename project layer");
    assert_eq!(renamed.project_layers.layers[1].name, "Primary folds");

    let presented = update_project_layer_presentation_in_project(
        &mut project,
        project_instance_id,
        project_id,
        3,
        crease_layer,
        ProjectLayerPresentationInput {
            visible: false,
            locked: true,
            opacity: 0.4,
        },
    )
    .expect("update project layer presentation");
    let presented_layer = presented
        .project_layers
        .layers
        .iter()
        .find(|layer| layer.id == crease_layer)
        .expect("presented layer");
    assert!(!presented_layer.visible);
    assert!(presented_layer.locked);
    assert_eq!(presented_layer.opacity, 0.4);

    let unlocked = update_project_layer_presentation_in_project(
        &mut project,
        project_instance_id,
        project_id,
        4,
        crease_layer,
        ProjectLayerPresentationInput {
            visible: true,
            locked: false,
            opacity: 0.4,
        },
    )
    .expect("unlock project layer");
    assert!(!unlocked.project_layers.layers[1].locked);

    let moved = move_project_layer_in_project(
        &mut project,
        project_instance_id,
        project_id,
        5,
        annotation_layer,
        0,
    )
    .expect("move project layer");
    assert_eq!(moved.project_layers.layers[0].id, annotation_layer);

    let assigned = assign_edge_to_project_layer_in_project(
        &mut project,
        project_instance_id,
        project_id,
        6,
        edge,
        crease_layer,
    )
    .expect("assign selected edge to crease-pattern layer");
    assert_eq!(assigned.project_layers.layer_for_edge(edge), crease_layer);

    let deleted = delete_project_layer_in_project(
        &mut project,
        project_instance_id,
        project_id,
        7,
        crease_layer,
    )
    .expect("delete project layer");
    assert_eq!(
        deleted.project_layers.layer_for_edge(edge),
        ori_domain::DEFAULT_PROJECT_LAYER_ID
    );
    assert!(
        deleted
            .project_layers
            .layers
            .iter()
            .all(|layer| layer.id != crease_layer)
    );

    assert!(
        delete_project_layer_in_project(
            &mut project,
            project_instance_id,
            project_id,
            8,
            ori_domain::DEFAULT_PROJECT_LAYER_ID,
        )
        .is_err()
    );
    assert_eq!(project.editor.revision(), 8);
    assert_eq!(project.editor.project_layers(), &deleted.project_layers);
}

#[test]
fn project_layer_presentation_ipc_input_is_a_strict_nested_record() {
    let admitted = serde_json::from_value::<ProjectLayerPresentationInput>(serde_json::json!({
        "visible": false,
        "locked": true,
        "opacity": 0.4
    }))
    .expect("strict presentation input");
    assert!(!admitted.visible);
    assert!(admitted.locked);
    assert_eq!(admitted.opacity, 0.4);
    assert!(
        serde_json::from_value::<ProjectLayerPresentationInput>(serde_json::json!({
            "visible": false,
            "locked": true,
            "opacity": 0.4,
            "future": "rejected"
        }),)
        .is_err()
    );
    assert!(
        serde_json::from_value::<ProjectLayerPresentationInput>(serde_json::json!({
            "visible": false,
            "opacity": 0.4
        }),)
        .is_err()
    );
}

#[test]
fn geometric_constraint_preflight_exposes_exact_positive_and_fail_closed_states() {
    let project = initial_project_state();
    let pattern = project.editor.pattern();
    let first_edge = pattern.edges[0].id;
    let second_edge = pattern.edges[1].id;
    let horizontal = GeometricConstraintRecordV1 {
        id: ConstraintId::new(),
        constraint: GeometricConstraintKindV1::Horizontal { edge: first_edge },
    };

    let exact_positive = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![horizontal.clone()],
    };
    assert_eq!(
        analyze_geometric_constraint_document(pattern, &exact_positive),
        GeometricConstraintPreflightResult::ProvenSatisfiable {
            model_id: ori_core::GEOMETRIC_CONSTRAINT_CURRENT_RUNTIME_EXACT_SATISFACTION_MODEL_ID_V1,
            constraint_count: 1,
            equation_count: 1,
            authorizes_project_mutation: false,
            replayable_across_runtimes: false,
        }
    );
    assert_eq!(
        serde_json::to_value(analyze_geometric_constraint_document(
            pattern,
            &exact_positive,
        ))
        .expect("serialize exact positive constraint result"),
        serde_json::json!({
            "status": "proven_satisfiable",
            "model_id": "geometric_constraint_current_runtime_exact_satisfaction_v1",
            "constraint_count": 1,
            "equation_count": 1,
            "authorizes_project_mutation": false,
            "replayable_across_runtimes": false,
        })
    );

    let no_direct = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::Horizontal { edge: second_edge },
        }],
    };
    assert_eq!(
        analyze_geometric_constraint_document(pattern, &no_direct),
        GeometricConstraintPreflightResult::NoDirectConflict
    );

    let zero_length_escape = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![
            horizontal.clone(),
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Vertical { edge: first_edge },
            },
        ],
    };
    assert!(matches!(
        analyze_geometric_constraint_document(pattern, &zero_length_escape),
        GeometricConstraintPreflightResult::Unknown {
            reason: GeometricConstraintUnknownReason::SolverRequiredConstraintKinds,
            ref unchecked_constraint_ids,
        } if unchecked_constraint_ids.len() == 2
    ));

    let direct = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![
            horizontal,
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Vertical { edge: first_edge },
            },
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::FixedLength {
                    edge: first_edge,
                    length_mm: 1.0,
                },
            },
        ],
    };
    let mut expected_mus_ids = direct
        .constraints
        .iter()
        .map(|record| record.id)
        .collect::<Vec<_>>();
    expected_mus_ids.sort_unstable_by_key(ConstraintId::canonical_bytes);
    let GeometricConstraintPreflightResult::DirectConflict {
        conflicts,
        bounded_direct_mus,
    } = analyze_geometric_constraint_document(pattern, &direct)
    else {
        panic!("horizontal plus vertical must be a direct conflict");
    };
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].constraint_ids().len(), 3);
    assert_eq!(
        bounded_direct_mus,
        BoundedDirectMusResult::ProvenUnsatisfiable {
            constraint_ids: expected_mus_ids.clone(),
            oracle_calls: 7,
        }
    );
    assert_eq!(
        serde_json::to_value(&bounded_direct_mus)
            .expect("serialize the bounded direct-conflict result"),
        serde_json::json!({
            "status": "proven_unsatisfiable",
            "constraint_ids": expected_mus_ids,
            "oracle_calls": 7,
        })
    );

    let solver_required = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::LengthRatio {
                numerator_edge: first_edge,
                denominator_edge: second_edge,
                ratio: 2.0,
            },
        }],
    };
    assert!(matches!(
        analyze_geometric_constraint_document(pattern, &solver_required),
        GeometricConstraintPreflightResult::Unknown {
            reason: GeometricConstraintUnknownReason::SolverRequiredConstraintKinds,
            ..
        }
    ));
}

#[test]
fn exact_positive_publication_rechecks_late_cancel_and_deadline() {
    let project = initial_project_state();
    let pattern = project.editor.pattern();
    let constraint_id = ConstraintId::new();
    let document = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![GeometricConstraintRecordV1 {
            id: constraint_id,
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: pattern.edges[0].id,
            },
        }],
    };
    let certificate =
        certify_binary64_exact_geometric_constraint_satisfaction_v1(pattern, &document)
            .expect("valid exact fixture")
            .expect("initial horizontal edge is exact");

    for (runtime, expected_reason) in [
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(true)),
                deadline: std::time::Instant::now()
                    .checked_add(Duration::from_secs(60))
                    .expect("future test deadline"),
            },
            GeometricConstraintUnknownReason::Cancelled,
        ),
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(false)),
                deadline: std::time::Instant::now(),
            },
            GeometricConstraintUnknownReason::DeadlineReached,
        ),
    ] {
        assert_eq!(
            crate::geometric_constraint_analysis::finish_exact_geometric_constraint_satisfaction(
                &document,
                &mut GeometricConstraintAnalysisObserver::new(runtime),
                certificate,
            ),
            GeometricConstraintPreflightResult::Unknown {
                reason: expected_reason,
                unchecked_constraint_ids: vec![constraint_id],
            }
        );
    }
}

#[test]
fn geometric_constraint_analysis_observer_reports_cancel_and_deadline_without_mutation() {
    let project = initial_project_state();
    let pattern = project.editor.pattern();
    let constraint_id = ConstraintId::new();
    let document = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![GeometricConstraintRecordV1 {
            id: constraint_id,
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: pattern.edges[0].id,
            },
        }],
    };
    let pattern_before = pattern.clone();
    let document_before = document.clone();

    for (runtime, expected_reason, expected_wire_reason) in [
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(true)),
                deadline: std::time::Instant::now()
                    .checked_add(Duration::from_secs(60))
                    .expect("future test deadline"),
            },
            GeometricConstraintUnknownReason::Cancelled,
            "cancelled",
        ),
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(false)),
                deadline: std::time::Instant::now(),
            },
            GeometricConstraintUnknownReason::DeadlineReached,
            "deadline_reached",
        ),
    ] {
        let result = analyze_geometric_constraint_document_with_observer(
            pattern,
            &document,
            &mut GeometricConstraintAnalysisObserver::new(runtime),
        );
        assert_eq!(
            result,
            GeometricConstraintPreflightResult::Unknown {
                reason: expected_reason,
                unchecked_constraint_ids: vec![constraint_id],
            }
        );
        assert_eq!(
            serde_json::to_value(&result)
                .expect("serialize stopped geometric-constraint preflight")["reason"],
            expected_wire_reason
        );
    }
    assert_eq!(pattern, &pattern_before);
    assert_eq!(document, document_before);
}

#[test]
fn bounded_direct_mus_reports_cancel_and_deadline_as_distinct_unknown_reasons() {
    let project = initial_project_state();
    let pattern = project.editor.pattern();
    let edge = pattern.edges[0].id;
    let document = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Horizontal { edge },
            },
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Vertical { edge },
            },
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::FixedLength {
                    edge,
                    length_mm: 1.0,
                },
            },
        ],
    };
    let prepared = prepare_geometric_constraints_v1(
        pattern,
        &document,
        GeometricConstraintLimitsV1::default(),
    )
    .expect("prepare direct-conflict MUS fixture");

    for (runtime, expected_reason, expected_wire_reason) in [
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(true)),
                deadline: std::time::Instant::now()
                    .checked_add(Duration::from_secs(60))
                    .expect("future test deadline"),
            },
            BoundedDirectMusUnknownReason::Cancelled,
            "cancelled",
        ),
        (
            GeometricConstraintAnalysisRuntime {
                cancellation: Arc::new(AtomicBool::new(false)),
                deadline: std::time::Instant::now(),
            },
            BoundedDirectMusUnknownReason::DeadlineReached,
            "deadline_reached",
        ),
    ] {
        let result = analyze_bounded_direct_mus_with_observer(
            &prepared,
            &mut GeometricConstraintAnalysisObserver::new(runtime),
        );
        assert_eq!(
            result,
            BoundedDirectMusResult::Unknown {
                reason: expected_reason,
                oracle_calls: 0,
                max_constraints: MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1,
            }
        );
        assert_eq!(
            serde_json::to_value(&result).expect("serialize stopped bounded direct MUS")["reason"],
            expected_wire_reason
        );
    }
}

#[test]
fn geometric_constraint_direct_mus_honors_the_sixteen_constraint_boundary() {
    let project = initial_project_state();
    let pattern = project.editor.pattern();
    let first_edge = pattern.edges[0].id;

    for count in [16_usize, 17] {
        let mut constraints = vec![
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Horizontal { edge: first_edge },
            },
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::Vertical { edge: first_edge },
            },
            GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::FixedLength {
                    edge: first_edge,
                    length_mm: 1.0,
                },
            },
        ];
        constraints.extend((3..count).map(|_| GeometricConstraintRecordV1 {
            id: ConstraintId::new(),
            constraint: GeometricConstraintKindV1::FixedLength {
                edge: first_edge,
                length_mm: 1.0,
            },
        }));
        let document = GeometricConstraintDocumentV1 {
            schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
            constraints,
        };

        let GeometricConstraintPreflightResult::DirectConflict {
            conflicts,
            bounded_direct_mus,
        } = analyze_geometric_constraint_document(pattern, &document)
        else {
            panic!("the direct conflict must remain visible at the MUS size boundary");
        };

        assert!(!conflicts.is_empty());
        if count == MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1 {
            let BoundedDirectMusResult::ProvenUnsatisfiable {
                constraint_ids,
                oracle_calls,
            } = bounded_direct_mus
            else {
                panic!("sixteen constraints must still run the bounded direct oracle");
            };
            assert_eq!(constraint_ids.len(), 3);
            assert!((1..=ori_core::MAX_BOUNDED_DIRECT_MUS_ORACLE_CALLS_V1).contains(&oracle_calls));
        } else {
            assert_eq!(
                bounded_direct_mus,
                BoundedDirectMusResult::Unknown {
                    reason: BoundedDirectMusUnknownReason::ConstraintLimitExceeded,
                    oracle_calls: 0,
                    max_constraints: MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1,
                }
            );
            assert_eq!(
                serde_json::to_value(&bounded_direct_mus)
                    .expect("serialize the skipped bounded direct-conflict result"),
                serde_json::json!({
                    "status": "unknown",
                    "reason": "constraint_limit_exceeded",
                    "oracle_calls": 0,
                    "max_constraints": MAX_BOUNDED_DIRECT_MUS_CONSTRAINTS_V1,
                })
            );
        }
    }
}

fn oversized_geometric_constraint_vertex_pattern() -> CreasePattern {
    let vertices = (0..=ori_domain::DEFAULT_MAX_CONSTRAINT_VERTICES)
        .map(|index| Vertex {
            id: VertexId::new(),
            position: Point2::new(index as f64, (index % 2) as f64),
        })
        .collect::<Vec<_>>();
    let edges = vec![Edge {
        id: EdgeId::new(),
        start: vertices[0].id,
        end: vertices[1].id,
        kind: EdgeKind::Mountain,
    }];
    CreasePattern { vertices, edges }
}

#[test]
fn geometric_constraint_empty_v1_preflight_skips_oversized_and_repair_geometry() {
    let empty = GeometricConstraintDocumentV1::default();
    let empty_before = empty.clone();
    let oversized = oversized_geometric_constraint_vertex_pattern();
    let oversized_before = oversized.clone();

    assert_eq!(oversized.vertices.len(), 100_001);
    assert_eq!(
        analyze_geometric_constraint_document(&oversized, &empty),
        GeometricConstraintPreflightResult::NoDirectConflict
    );
    assert_eq!(oversized, oversized_before);
    assert_eq!(empty, empty_before);

    let duplicate_vertex = VertexId::new();
    let repair_geometry = CreasePattern {
        vertices: vec![
            Vertex {
                id: duplicate_vertex,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: duplicate_vertex,
                position: Point2::new(1.0, 0.0),
            },
        ],
        edges: vec![Edge {
            id: EdgeId::new(),
            start: duplicate_vertex,
            end: VertexId::new(),
            kind: EdgeKind::Valley,
        }],
    };
    let repair_geometry_before = repair_geometry.clone();

    assert_eq!(
        analyze_geometric_constraint_document(&repair_geometry, &empty),
        GeometricConstraintPreflightResult::NoDirectConflict
    );
    assert_eq!(repair_geometry, repair_geometry_before);
    assert_eq!(empty, empty_before);
}

#[test]
fn geometric_constraint_empty_invalid_schema_remains_unknown() {
    let pattern = CreasePattern::empty();
    let invalid = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1 + 1,
        constraints: Vec::new(),
    };
    let pattern_before = pattern.clone();
    let invalid_before = invalid.clone();

    assert_eq!(
        analyze_geometric_constraint_document(&pattern, &invalid),
        GeometricConstraintPreflightResult::Unknown {
            reason: GeometricConstraintUnknownReason::InvalidDocumentOrGeometry,
            unchecked_constraint_ids: Vec::new(),
        }
    );
    assert_eq!(pattern, pattern_before);
    assert_eq!(invalid, invalid_before);
}

#[test]
fn geometric_constraint_non_empty_oversized_geometry_remains_unknown() {
    let pattern = oversized_geometric_constraint_vertex_pattern();
    let constraint_id = ConstraintId::new();
    let document = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![GeometricConstraintRecordV1 {
            id: constraint_id,
            constraint: GeometricConstraintKindV1::Horizontal {
                edge: pattern.edges[0].id,
            },
        }],
    };
    let pattern_before = pattern.clone();
    let document_before = document.clone();

    assert_eq!(
        analyze_geometric_constraint_document(&pattern, &document),
        GeometricConstraintPreflightResult::Unknown {
            reason: GeometricConstraintUnknownReason::InvalidDocumentOrGeometry,
            unchecked_constraint_ids: vec![constraint_id],
        }
    );
    assert_eq!(pattern, pattern_before);
    assert_eq!(document, document_before);
}

#[test]
fn geometric_constraint_preflight_fails_closed_for_invalid_references() {
    let project = initial_project_state();
    let first = ConstraintId::new();
    let second = ConstraintId::new();
    let invalid = GeometricConstraintDocumentV1 {
        schema_version: ori_domain::GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
        constraints: vec![
            GeometricConstraintRecordV1 {
                id: first,
                constraint: GeometricConstraintKindV1::Horizontal {
                    edge: EdgeId::new(),
                },
            },
            GeometricConstraintRecordV1 {
                id: second,
                constraint: GeometricConstraintKindV1::Vertical {
                    edge: EdgeId::new(),
                },
            },
        ],
    };

    let GeometricConstraintPreflightResult::Unknown {
        reason,
        unchecked_constraint_ids,
    } = analyze_geometric_constraint_document(project.editor.pattern(), &invalid)
    else {
        panic!("invalid references must not be reported as safe");
    };
    assert_eq!(
        reason,
        GeometricConstraintUnknownReason::InvalidDocumentOrGeometry
    );
    let mut expected = vec![first, second];
    expected.sort_unstable_by_key(ConstraintId::canonical_bytes);
    assert_eq!(unchecked_constraint_ids, expected);
}

#[test]
fn geometric_constraint_worker_gate_is_exclusive_and_releases_with_its_permit() {
    let gate = GeometricConstraintWorkerGate::default();
    let binding = GeometricConstraintAnalysisBinding {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 7,
    };
    let request_generation_id = ProjectId::new();
    assert!(!gate.is_busy());
    assert_eq!(gate.pre_cancelled_count(), 0);
    let permit = gate
        .try_acquire(binding, request_generation_id)
        .expect("first worker permit");
    assert!(gate.is_busy());
    assert!(
        gate.try_acquire(binding, ProjectId::new()).is_none(),
        "parallel preflight must not allocate another worker"
    );
    assert!(
        !gate.cancel(
            GeometricConstraintAnalysisBinding {
                revision: binding.revision + 1,
                ..binding
            },
            request_generation_id,
        ),
        "a stale binding must not cancel the active worker"
    );
    assert!(
        !gate.cancel(binding, ProjectId::new()),
        "a stale request generation must not cancel the active worker"
    );
    assert!(!permit.cancellation.load(Ordering::Acquire));
    assert!(gate.cancel(binding, request_generation_id));
    assert!(permit.cancellation.load(Ordering::Acquire));
    drop(permit);
    assert!(!gate.is_busy());
    assert!(
        gate.try_acquire(binding, ProjectId::new()).is_some(),
        "the released gate must admit the next request generation"
    );
}

#[test]
fn geometric_constraint_gate_consumes_exact_cancel_before_acquire_once() {
    let gate = GeometricConstraintWorkerGate::default();
    let binding = GeometricConstraintAnalysisBinding {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 11,
    };
    let request_generation_id = ProjectId::new();

    assert!(gate.cancel(binding, request_generation_id));
    assert!(gate.cancel(binding, request_generation_id));
    assert_eq!(
        gate.pre_cancelled_count(),
        1,
        "duplicate early cancellation must occupy one bounded slot"
    );
    let cancelled = gate
        .try_acquire(binding, request_generation_id)
        .expect("the matching request must still acquire the worker slot");
    assert!(cancelled.cancellation.load(Ordering::Acquire));
    assert_eq!(gate.pre_cancelled_count(), 0);
    drop(cancelled);

    let next_generation = gate
        .try_acquire(binding, ProjectId::new())
        .expect("the next generation must acquire independently");
    assert!(
        !next_generation.cancellation.load(Ordering::Acquire),
        "an early cancellation must be consumed only by its exact generation"
    );
}

#[test]
fn geometric_constraint_gate_retains_queued_cancel_while_another_generation_is_active() {
    let gate = GeometricConstraintWorkerGate::default();
    let binding = GeometricConstraintAnalysisBinding {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 12,
    };
    let active_generation = ProjectId::new();
    let queued_generation = ProjectId::new();
    let active = gate
        .try_acquire(binding, active_generation)
        .expect("the first generation must acquire");

    assert!(
        !gate.cancel(binding, queued_generation),
        "the queued generation is not the currently active worker"
    );
    assert!(
        !active.cancellation.load(Ordering::Acquire),
        "a queued generation must not cancel the active generation"
    );
    assert_eq!(gate.pre_cancelled_count(), 1);
    drop(active);

    let queued = gate
        .try_acquire(binding, queued_generation)
        .expect("the queued generation must acquire after the active worker exits");
    assert!(
        queued.cancellation.load(Ordering::Acquire),
        "cancel arriving before the queued analyze future is first polled must be retained"
    );
    assert_eq!(gate.pre_cancelled_count(), 0);
}

#[test]
fn geometric_constraint_pre_cancel_ledger_is_bounded_and_evicts_oldest_only() {
    let gate = GeometricConstraintWorkerGate::default();
    let binding = GeometricConstraintAnalysisBinding {
        project_instance_id: ProjectId::new(),
        project_id: ProjectId::new(),
        revision: 13,
    };
    let request_generations = (0..=MAX_GEOMETRIC_CONSTRAINT_PRE_CANCELLED_REQUESTS)
        .map(|_| ProjectId::new())
        .collect::<Vec<_>>();
    for request_generation_id in &request_generations {
        assert!(gate.cancel(binding, *request_generation_id));
    }
    assert_eq!(
        gate.pre_cancelled_count(),
        MAX_GEOMETRIC_CONSTRAINT_PRE_CANCELLED_REQUESTS
    );

    let evicted = gate
        .try_acquire(binding, request_generations[0])
        .expect("the oldest evicted generation can acquire normally");
    assert!(!evicted.cancellation.load(Ordering::Acquire));
    drop(evicted);
    let newest = gate
        .try_acquire(
            binding,
            *request_generations
                .last()
                .expect("at least one request generation"),
        )
        .expect("the newest retained generation can acquire");
    assert!(newest.cancellation.load(Ordering::Acquire));
}

#[test]
fn geometric_constraint_gate_publishes_each_successful_acquire_before_cancel_can_observe_it() {
    for revision in 0..128 {
        let gate = GeometricConstraintWorkerGate::default();
        let binding = GeometricConstraintAnalysisBinding {
            project_instance_id: ProjectId::new(),
            project_id: ProjectId::new(),
            revision,
        };
        let request_generation_id = ProjectId::new();
        let worker_gate = gate.clone();
        let (acquired_tx, acquired_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        let worker = thread::spawn(move || {
            let permit = worker_gate
                .try_acquire(binding, request_generation_id)
                .expect("the fresh gate must admit one worker");
            acquired_tx
                .send(permit.cancellation())
                .expect("publish acquired cancellation token");
            release_rx.recv().expect("release acquired worker permit");
            drop(permit);
        });
        let cancellation = acquired_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the worker must publish its successful acquisition");

        assert!(
            gate.cancel(binding, request_generation_id),
            "a published successful acquisition must always be cancellable"
        );
        assert!(cancellation.load(Ordering::Acquire));
        release_tx.send(()).expect("release worker");
        worker.join().expect("worker must not panic");
        assert!(!gate.is_busy());
    }
}

#[test]
fn geometric_constraint_worker_cancel_is_bound_to_exact_request_generation() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let binding_tuple = geometric_constraint_binding(&state);
    let binding = GeometricConstraintAnalysisBinding {
        project_instance_id: binding_tuple.0,
        project_id: binding_tuple.1,
        revision: binding_tuple.2,
    };
    let request_generation_id = ProjectId::new();
    let before = geometric_constraint_project_signature(&state);
    let worker_state = Arc::clone(&state);
    let (entered_tx, entered_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);

    let worker = thread::spawn(move || {
        tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
            &worker_state,
            binding.project_instance_id,
            binding.project_id,
            binding.revision,
            request_generation_id,
            move |pattern, document, runtime| {
                entered_tx.send(()).expect("announce worker entry");
                release_rx.recv().expect("release constraint worker");
                Ok(analyze_geometric_constraint_document_with_observer(
                    &pattern,
                    &document,
                    &mut GeometricConstraintAnalysisObserver::new(runtime),
                ))
            },
        ))
    });
    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("geometric-constraint worker must start");

    assert!(
        !state.cancel_geometric_constraint_worker(binding, ProjectId::new()),
        "a stale generation must not cancel the current worker"
    );
    assert!(
        !state.cancel_geometric_constraint_worker(
            GeometricConstraintAnalysisBinding {
                revision: binding.revision + 1,
                ..binding
            },
            request_generation_id,
        ),
        "a stale binding must not cancel the current worker"
    );
    assert!(
        state.cancel_geometric_constraint_worker(binding, request_generation_id),
        "the exact binding and request generation must cancel the worker"
    );
    release_tx.send(()).expect("release cancelled worker");
    let response = worker
        .join()
        .expect("analysis caller must not panic")
        .expect("cancelled analysis returns a bound fail-closed result");

    assert_eq!(response.project_instance_id, binding.project_instance_id);
    assert_eq!(response.project_id, binding.project_id);
    assert_eq!(response.revision, binding.revision);
    assert!(matches!(
        response.result,
        GeometricConstraintPreflightResult::Unknown {
            reason: GeometricConstraintUnknownReason::Cancelled,
            ..
        }
    ));
    assert!(!state.geometric_constraint_worker_is_busy());
    assert_eq!(geometric_constraint_project_signature(&state), before);
}

#[test]
fn abandoned_geometric_constraint_waiter_keeps_gate_until_worker_exit_then_retries() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let binding = geometric_constraint_binding(&state);
    let before = geometric_constraint_project_signature(&state);
    let worker_state = Arc::clone(&state);
    let (entered_tx, entered_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);

    let waiting = tauri::async_runtime::spawn(async move {
        analyze_geometric_constraints_with_worker(
            &worker_state,
            binding.0,
            binding.1,
            binding.2,
            ProjectId::new(),
            move |pattern, document, _runtime| {
                entered_tx.send(()).expect("announce worker entry");
                release_rx.recv().expect("release constraint worker");
                Ok(analyze_geometric_constraint_document(&pattern, &document))
            },
        )
        .await
    });

    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("geometric-constraint worker must start");
    assert!(state.geometric_constraint_worker_is_busy());
    waiting.abort();
    assert!(
        tauri::async_runtime::block_on(waiting).is_err(),
        "the abandoned waiting future must be cancelled"
    );
    assert!(
        state.geometric_constraint_worker_is_busy(),
        "cancelling the waiter must not release a running blocking worker"
    );

    let busy_error = tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
        &state,
        binding.0,
        binding.1,
        binding.2,
        ProjectId::new(),
        |_, _, _runtime| {
            panic!("a busy gate must reject before invoking another worker");
        },
    ))
    .expect_err("parallel analysis must be rejected");
    assert_eq!(busy_error, GEOMETRIC_CONSTRAINT_ANALYSIS_BUSY_MESSAGE);

    release_tx
        .send(())
        .expect("release abandoned geometric-constraint worker");
    wait_for_geometric_constraint_worker_idle(&state);
    assert!(!state.geometric_constraint_worker_is_busy());

    let retried = run_default_geometric_constraint_analysis(&state, binding)
        .expect("the gate must be reusable after the blocking worker exits");
    assert_eq!(retried.project_instance_id, binding.0);
    assert_eq!(retried.project_id, binding.1);
    assert_eq!(retried.revision, binding.2);
    assert_eq!(
        retried.result,
        GeometricConstraintPreflightResult::NoDirectConflict
    );
    assert_eq!(geometric_constraint_project_signature(&state), before);
}

#[test]
fn geometric_constraint_worker_releases_project_lock_and_discards_reopen_aba_completion() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let stale_binding = geometric_constraint_binding(&state);
    let document = {
        let project = lock_project(&state).expect("capture original project document");
        project.document()
    };
    let worker_state = Arc::clone(&state);
    let (entered_tx, entered_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);

    let analysis = thread::spawn(move || {
        tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
            &worker_state,
            stale_binding.0,
            stale_binding.1,
            stale_binding.2,
            ProjectId::new(),
            move |pattern, constraints, _runtime| {
                entered_tx.send(()).expect("announce worker entry");
                release_rx.recv().expect("release constraint worker");
                Ok(analyze_geometric_constraint_document(
                    &pattern,
                    &constraints,
                ))
            },
        ))
    });

    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("geometric-constraint worker must start");
    let (current_binding, reopened_before) = {
        let Ok(mut project) = state.0.try_lock() else {
            release_tx
                .send(())
                .expect("release blocked geometric-constraint worker");
            analysis
                .join()
                .expect("analysis caller must not panic")
                .expect("unchanged analysis must finish");
            panic!("the project lock must be released during constraint analysis");
        };
        *project =
            ProjectState::from_valid_document(document, PathBuf::from("same-constraints.ori2"));
        assert_eq!(project.project_id, stale_binding.1);
        assert_eq!(project.editor.revision(), stale_binding.2);
        assert_ne!(project.instance_id, stale_binding.0);
        (
            (
                project.instance_id,
                project.project_id,
                project.editor.revision(),
            ),
            project_state_signature(&project),
        )
    };

    release_tx
        .send(())
        .expect("release stale geometric-constraint worker");
    let stale_error = analysis
        .join()
        .expect("analysis caller must not panic")
        .expect_err("same-ID and revision reopen must reject stale completion");
    assert_eq!(
        stale_error,
        "the open project instance changed while the file dialog was open"
    );
    assert!(!state.geometric_constraint_worker_is_busy());
    assert_eq!(
        geometric_constraint_project_signature(&state),
        reopened_before
    );

    let retried = run_default_geometric_constraint_analysis(&state, current_binding)
        .expect("the reopened instance must be able to retry");
    assert_eq!(retried.project_instance_id, current_binding.0);
    assert_eq!(retried.project_id, current_binding.1);
    assert_eq!(retried.revision, current_binding.2);
    assert_eq!(
        geometric_constraint_project_signature(&state),
        reopened_before
    );
}

#[test]
fn geometric_constraint_worker_failures_are_redacted_release_gate_and_preserve_state() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let binding = geometric_constraint_binding(&state);
    let before = geometric_constraint_project_signature(&state);
    let private_failure = r"C:\Users\alice\private-constraints.ori2; constraint_id=secret-17";

    let reported_error = tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
        &state,
        binding.0,
        binding.1,
        binding.2,
        ProjectId::new(),
        move |_, _, _runtime| Err(private_failure.to_owned()),
    ))
    .expect_err("a reported worker failure must fail the command");
    assert_eq!(reported_error, GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE);
    assert!(!reported_error.contains("alice"));
    assert!(!reported_error.contains("private-constraints"));
    assert!(!reported_error.contains("secret-17"));
    assert!(!state.geometric_constraint_worker_is_busy());
    assert_eq!(geometric_constraint_project_signature(&state), before);
    run_default_geometric_constraint_analysis(&state, binding)
        .expect("the gate must be reusable after a reported worker failure");

    let private_panic = r"C:\Users\bob\private-constraints.ori2; constraint_id=panic-secret-23";
    let panic_error = tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
        &state,
        binding.0,
        binding.1,
        binding.2,
        ProjectId::new(),
        move |_, _, _runtime| -> Result<GeometricConstraintPreflightResult, String> {
            panic!("{private_panic}");
        },
    ))
    .expect_err("a panicking worker must fail the command");
    assert_eq!(panic_error, GEOMETRIC_CONSTRAINT_ANALYSIS_FAILED_MESSAGE);
    assert!(!panic_error.contains("bob"));
    assert!(!panic_error.contains("private-constraints"));
    assert!(!panic_error.contains("panic-secret-23"));
    assert!(!state.geometric_constraint_worker_is_busy());
    assert_eq!(geometric_constraint_project_signature(&state), before);
    run_default_geometric_constraint_analysis(&state, binding)
        .expect("the gate must be reusable after a panicking worker");
    assert_eq!(geometric_constraint_project_signature(&state), before);
}

#[test]
fn geometric_constraint_capture_rejections_and_success_all_release_gate() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let binding = geometric_constraint_binding(&state);
    let before = geometric_constraint_project_signature(&state);
    let rejection_cases = [
        (
            (ProjectId::new(), binding.1, binding.2),
            "the open project instance changed while the file dialog was open",
        ),
        (
            (binding.0, ProjectId::new(), binding.2),
            "the active project changed before the command was applied",
        ),
        (
            (binding.0, binding.1, binding.2 + 1),
            "the project changed while the file dialog was open",
        ),
    ];

    for (rejected_binding, expected_error) in rejection_cases {
        let error = tauri::async_runtime::block_on(analyze_geometric_constraints_with_worker(
            &state,
            rejected_binding.0,
            rejected_binding.1,
            rejected_binding.2,
            ProjectId::new(),
            |_, _, _runtime| {
                panic!("capture rejection must happen before worker invocation");
            },
        ))
        .expect_err("invalid capture binding must be rejected");
        assert_eq!(error, expected_error);
        assert!(!state.geometric_constraint_worker_is_busy());
        assert_eq!(geometric_constraint_project_signature(&state), before);
    }

    let response = run_default_geometric_constraint_analysis(&state, binding)
        .expect("a valid capture and worker must succeed");
    assert_eq!(response.project_instance_id, binding.0);
    assert_eq!(response.project_id, binding.1);
    assert_eq!(response.revision, binding.2);
    assert!(!state.geometric_constraint_worker_is_busy());
    assert_eq!(geometric_constraint_project_signature(&state), before);
}

#[test]
fn lock_and_expect_preserves_project_expectation_order_and_errors() {
    let state = AppState::new(initial_project_state());
    let binding = {
        let project = lock_project(&state).expect("project lock");
        ProjectExpectation::new(
            project.instance_id,
            project.project_id,
            project.editor.revision(),
        )
    };

    let project = lock_and_expect(&state, binding).expect("matching expectation");
    assert_eq!(project.instance_id, binding.instance_id);
    assert_eq!(project.project_id, binding.project_id);
    assert_eq!(project.editor.revision(), binding.revision);
    drop(project);

    let Err(instance_error) = lock_and_expect(
        &state,
        ProjectExpectation::new(ProjectId::new(), binding.project_id, binding.revision),
    ) else {
        panic!("instance mismatch must fail");
    };
    assert_eq!(
        instance_error,
        "the open project instance changed while the file dialog was open"
    );

    let Err(project_error) = lock_and_expect(
        &state,
        ProjectExpectation::new(binding.instance_id, ProjectId::new(), binding.revision),
    ) else {
        panic!("project mismatch must fail");
    };
    assert_eq!(
        project_error,
        "the active project changed before the command was applied"
    );

    let Err(revision_error) = lock_and_expect(
        &state,
        ProjectExpectation::new(
            binding.instance_id,
            binding.project_id,
            binding.revision + 1,
        ),
    ) else {
        panic!("revision mismatch must fail");
    };
    assert_eq!(
        revision_error,
        "the project changed while the file dialog was open"
    );
}

#[test]
fn topology_bridge_returns_revision_bound_boundary_snapshot_without_mutation() {
    let project = initial_project_state();
    let before = project_state_signature(&project);
    let input =
        capture_topology_input(&project, project.project_id, 0).expect("capture initial sheet");
    let topology = input.analyze();

    let response =
        finish_topology_response(&project, &input, topology).expect("finish current topology");

    assert_eq!(response.project_id, project.project_id);
    assert_eq!(response.revision, 0);
    assert!(response.simulation_ready);
    assert!(response.issues.is_empty());
    let snapshot = response.snapshot.expect("boundary snapshot");
    assert_eq!(snapshot.source_revision, response.revision);
    assert_eq!(snapshot.faces.len(), 1);
    assert!(snapshot.hinge_adjacency.is_empty());
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn topology_bridge_returns_two_faces_and_one_hinge_for_one_fold() {
    let mut project = initial_project_state();
    let fold = EdgeId::new();
    let endpoints = [
        project.editor.paper().boundary_vertices[0],
        project.editor.paper().boundary_vertices[2],
    ];
    let project_id = project.project_id;
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddEdge {
            id: fold,
            start: endpoints[0],
            end: endpoints[1],
            kind: EdgeKind::Mountain,
        },
    )
    .expect("add one fold");
    let before = project_state_signature(&project);
    let input = capture_topology_input(&project, project_id, 1).expect("capture fold");

    let response =
        finish_topology_response(&project, &input, input.analyze()).expect("finish fold topology");

    assert!(response.simulation_ready);
    assert!(response.issues.is_empty());
    let snapshot = response.snapshot.expect("fold snapshot");
    assert_eq!(snapshot.source_revision, 1);
    assert_eq!(snapshot.faces.len(), 2);
    assert_eq!(snapshot.hinge_adjacency.len(), 1);
    assert_eq!(snapshot.hinge_adjacency[0].edge, fold);
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn instruction_pose_accepts_planar_and_complete_tree_models() {
    let project = initial_project_state();
    let input = capture_topology_input(&project, project.project_id, 0)
        .expect("capture planar instruction model");
    let topology = input.analyze();
    let planar = instruction_pose_from_topology(
        topology
            .simulation_snapshot()
            .expect("planar topology must be simulation-ready"),
        "0".repeat(64),
        None,
        Vec::new(),
    )
    .expect("accept planar instruction pose");
    assert_eq!(planar.fixed_face, None);
    assert!(planar.hinge_angles.is_empty());

    let mut folded = initial_project_state();
    let fold = EdgeId::new();
    let boundary = folded.editor.paper().boundary_vertices.clone();
    let project_id = folded.project_id;
    execute_command(
        &mut folded,
        project_id,
        0,
        Command::AddEdge {
            id: fold,
            start: boundary[0],
            end: boundary[2],
            kind: EdgeKind::Mountain,
        },
    )
    .expect("add one instruction hinge");
    let input = capture_topology_input(&folded, project_id, 1).expect("capture fold model");
    let topology = input.analyze();
    let snapshot = topology
        .simulation_snapshot()
        .expect("one-fold topology must be simulation-ready");
    let fixed_face = snapshot.faces[0].id;
    let pose = instruction_pose_from_topology(
        snapshot,
        folded.editor.fold_model_fingerprint_v1(),
        Some(fixed_face),
        vec![InstructionHingeAngle {
            edge: fold,
            angle_degrees: 37.5,
        }],
    )
    .expect("accept complete one-fold instruction pose");

    assert_eq!(pose.fixed_face, Some(fixed_face));
    assert_eq!(pose.hinge_angles.len(), 1);
    assert_eq!(pose.hinge_angles[0].edge, fold);
    assert_eq!(pose.hinge_angles[0].angle_degrees, 37.5);
    assert_eq!(
        pose.source_model_fingerprint,
        folded.editor.fold_model_fingerprint_v1()
    );
}

#[test]
fn instruction_pose_rejects_wrong_faces_incomplete_hinges_and_bad_angles() {
    let mut project = initial_project_state();
    let fold = EdgeId::new();
    let boundary = project.editor.paper().boundary_vertices.clone();
    let project_id = project.project_id;
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddEdge {
            id: fold,
            start: boundary[0],
            end: boundary[2],
            kind: EdgeKind::Valley,
        },
    )
    .expect("add one instruction hinge");
    let input = capture_topology_input(&project, project_id, 1).expect("capture fold model");
    let topology = input.analyze();
    let snapshot = topology
        .simulation_snapshot()
        .expect("one-fold topology must be simulation-ready");
    let fingerprint = project.editor.fold_model_fingerprint_v1();

    assert_eq!(
        instruction_pose_from_topology(
            snapshot,
            fingerprint.clone(),
            None,
            vec![InstructionHingeAngle {
                edge: fold,
                angle_degrees: 45.0,
            }],
        )
        .expect_err("a folded pose needs a fixed face"),
        "a folded instruction pose requires a fixed face"
    );
    assert_eq!(
        instruction_pose_from_topology(
            snapshot,
            fingerprint.clone(),
            Some(FaceId::new()),
            vec![InstructionHingeAngle {
                edge: fold,
                angle_degrees: 45.0,
            }],
        )
        .expect_err("the fixed face must be current"),
        "the fixed face does not exist in the current fold model"
    );
    assert_eq!(
        instruction_pose_from_topology(
            snapshot,
            fingerprint.clone(),
            Some(snapshot.faces[0].id),
            Vec::new(),
        )
        .expect_err("every hinge is required"),
        "the instruction pose must contain every current hinge exactly once"
    );
    assert_eq!(
        instruction_pose_from_topology(
            snapshot,
            fingerprint,
            Some(snapshot.faces[0].id),
            vec![InstructionHingeAngle {
                edge: fold,
                angle_degrees: f64::NAN,
            }],
        )
        .expect_err("non-finite angles are rejected"),
        "instruction hinge angles must be finite"
    );
}

#[test]
fn instruction_pose_rejects_fold_graph_cycles() {
    let (project, _) = four_ray_square_project_state(
        [1, 3, 5, 7],
        [
            EdgeKind::Mountain,
            EdgeKind::Valley,
            EdgeKind::Mountain,
            EdgeKind::Valley,
        ],
    );
    let input =
        capture_topology_input(&project, project.project_id, 0).expect("capture cyclic fold model");
    let topology = input.analyze();
    let snapshot = topology
        .simulation_snapshot()
        .expect("the topology layer admits the cyclic model");
    let hinge_angles = snapshot
        .hinge_adjacency
        .iter()
        .map(|hinge| InstructionHingeAngle {
            edge: hinge.edge,
            angle_degrees: 0.0,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        instruction_pose_from_topology(
            snapshot,
            project.editor.fold_model_fingerprint_v1(),
            Some(snapshot.faces[0].id),
            hinge_angles,
        )
        .expect_err("the first instruction player supports trees only"),
        "instruction poses currently require a tree-shaped fold graph"
    );
}

#[test]
fn beginner_cyclic_path_certificate_is_bound_across_supported_thicknesses() {
    let mut thickness_certificates = Vec::new();
    for thickness_mm in [0.0, 0.1, 1.0, 3.0] {
        let fixture_namespace: ProjectId =
            serde_json::from_str("\"01900000-0000-7000-8000-000000000497\"")
                .expect("fixed cross-platform fixture namespace");
        let points = [
            (100.0, 0.0),
            (-50.0, 86.602_540_378_443_86),
            (-50.0, -86.602_540_378_443_86),
            (50.0, -86.602_540_378_443_86),
            (0.0, 0.0),
        ];
        let vertices = points
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: VertexId::derive_v5(fixture_namespace, format!("vertex-{index}").as_bytes()),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices[..4]
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let center = vertices[4].id;
        let mut fold_ids = (0_u64..4)
            .map(|index| EdgeId::derive_v5(fixture_namespace, &index.to_be_bytes()))
            .collect::<Vec<_>>();
        fold_ids.sort_unstable_by_key(EdgeId::canonical_bytes);
        let mut edges = (0..4)
            .map(|index| Edge {
                id: EdgeId::derive_v5(fixture_namespace, format!("boundary-{index}").as_bytes()),
                start: boundary[index],
                end: boundary[(index + 1) % 4],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.extend((0..4).map(|index| Edge {
            id: fold_ids[index],
            start: boundary[index],
            end: center,
            kind: if index == 3 {
                EdgeKind::Mountain
            } else {
                EdgeKind::Valley
            },
        }));
        let pattern = CreasePattern { vertices, edges };
        let paper = Paper {
            boundary_vertices: boundary,
            thickness_mm,
            ..Paper::default()
        };
        let candidate_editor = EditorState::with_paper(pattern.clone(), paper.clone());
        let topology = candidate_editor
            .topology_analysis_input(fixture_namespace)
            .analyze();
        let topology = topology.simulation_snapshot().expect("cyclic topology");
        assert!(
            ori_kinematics::MaterialTreeKinematicsModel::prepare(
                &pattern,
                &paper,
                topology,
                ori_kinematics::TreeKinematicsLimits::default(),
            )
            .is_err(),
            "cyclic fixture must reject tree preparation at {thickness_mm} mm"
        );
        let geometry = ori_kinematics::MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            topology,
            ori_kinematics::TreeKinematicsLimits::default(),
        )
        .expect("cyclic geometry");
        let audit = ori_kinematics::MaterialHingeGraphAudit::prepare(
            topology,
            ori_kinematics::TreeKinematicsLimits::default(),
        )
        .expect("cyclic audit");
        let mut fixed_faces = geometry.face_ids().to_vec();
        fixed_faces.sort_unstable_by_key(|face| face.canonical_bytes());
        let positive_thickness_supported = fixed_faces.iter().any(|fixed| {
            ori_kinematics::generate_kawasaki_120_120_60_60_path_candidate_v1(
                &geometry,
                &audit,
                *fixed,
                ori_kinematics::CycleScheduleLimitsV1::default(),
            )
            .is_ok_and(|candidate| {
                ori_collision::supports_scheduled_positive_thickness_path_v1(
                    &geometry,
                    &audit,
                    *fixed,
                    candidate.schedule(),
                )
            })
        });
        let certificate = fixed_faces.into_iter().find_map(|fixed| {
            let generated = ori_kinematics::generate_kawasaki_120_120_60_60_path_candidate_v1(
                &geometry,
                &audit,
                fixed,
                ori_kinematics::CycleScheduleLimitsV1::default(),
            )
            .ok()?;
            let closure = geometry
                .prove_dyadic_schedule_closure_v1(
                    &audit,
                    fixed,
                    generated.schedule(),
                    1.0e-8,
                    ori_kinematics::DyadicIntervalClosureLimitsV1 {
                        max_depth: 16,
                        max_leaves: 65_536,
                        max_work: 1_048_576,
                        schedule_limits: ori_kinematics::CycleScheduleLimitsV1::default(),
                    },
                )
                .ok()?;
            let path = if thickness_mm > 0.0 {
                ori_collision::diagnose_scheduled_positive_thickness_cycle_path_v1(
                    &geometry,
                    &audit,
                    fixed,
                    &generated,
                    &closure,
                    thickness_mm,
                    32,
                )
            } else {
                ori_collision::diagnose_scheduled_cycle_path_v1(
                    &geometry, &audit, fixed, &generated, &closure, 32,
                )
            };
            path.continuous_certificate_model_id()
        });
        if let Some(certificate) = certificate {
            thickness_certificates.push(certificate);
        } else if thickness_mm > 0.0 && !positive_thickness_supported {
            assert!(certificate.is_none());
        }
        let original_pattern = pattern.clone();
        let original_paper = paper.clone();
        assert_eq!(
            pattern, original_pattern,
            "document pattern is observation-only"
        );
        assert_eq!(paper, original_paper, "document paper is observation-only");
    }
    let unique = thickness_certificates.iter().collect::<HashSet<_>>();
    assert_eq!(unique.len(), thickness_certificates.len());
}

#[test]
fn named_technique_timeline_proposal_is_strict_bounded_and_ordered() {
    let valid = serde_json::json!({
        "schema_version": 1,
        "package_id": "builtin.origami2",
        "technique_id": "inside-reverse",
        "technique_version": 1,
        "steps": [
            {
                "source_kind": "technique",
                "source_id": "inside-reverse",
                "chunk_index": 1,
                "chunk_count": 1,
                "title": "Technique",
                "description": "source-json-v1:\n{}",
                "caution": "description only",
                "duration_ms": 1500
            },
            {
                "source_kind": "operation",
                "source_id": "open",
                "chunk_index": 1,
                "chunk_count": 2,
                "title": "Operation (1/2)",
                "description": "first",
                "caution": "no physical command",
                "duration_ms": 1500
            },
            {
                "source_kind": "operation",
                "source_id": "open",
                "chunk_index": 2,
                "chunk_count": 2,
                "title": "Operation (2/2)",
                "description": "second",
                "caution": "no physical command",
                "duration_ms": 1500
            }
        ]
    });
    let proposal = parse_named_technique_timeline_proposal(
        &serde_json::to_string(&valid).expect("proposal JSON"),
    )
    .expect("valid proposal");
    assert_eq!(proposal.steps.len(), 3);

    let mut invalid_values = Vec::new();
    let mut unknown_root = valid.clone();
    unknown_root["private_path"] = serde_json::Value::String("secret".to_owned());
    invalid_values.push(unknown_root);
    let mut unknown_step = valid.clone();
    unknown_step["steps"][0]["fixed_face"] = serde_json::Value::Null;
    invalid_values.push(unknown_step);
    let mut wrong_first_kind = valid.clone();
    wrong_first_kind["steps"][0]["source_kind"] = serde_json::Value::String("operation".to_owned());
    invalid_values.push(wrong_first_kind);
    let mut wrong_technique_source = valid.clone();
    wrong_technique_source["steps"][0]["source_id"] = serde_json::Value::String("other".to_owned());
    invalid_values.push(wrong_technique_source);
    let mut incomplete_chunks = valid.clone();
    incomplete_chunks["steps"]
        .as_array_mut()
        .expect("steps")
        .pop();
    invalid_values.push(incomplete_chunks);
    let mut repeated_source = valid.clone();
    repeated_source["steps"]
        .as_array_mut()
        .expect("steps")
        .push(serde_json::json!({
            "source_kind": "operation",
            "source_id": "open",
            "chunk_index": 1,
            "chunk_count": 1,
            "title": "Repeated",
            "description": "repeated",
            "caution": "",
            "duration_ms": 1500
        }));
    invalid_values.push(repeated_source);
    let mut invalid_identifier = valid.clone();
    invalid_identifier["package_id"] = serde_json::Value::String("../private".to_owned());
    invalid_values.push(invalid_identifier);

    for invalid in invalid_values {
        assert_eq!(
            parse_named_technique_timeline_proposal(
                &serde_json::to_string(&invalid).expect("invalid fixture JSON"),
            )
            .expect_err("invalid proposal"),
            "the named-technique timeline proposal is invalid"
        );
    }
    assert_eq!(
        parse_named_technique_timeline_proposal(
            &" ".repeat(MAX_NAMED_TECHNIQUE_TIMELINE_PROPOSAL_BYTES + 1),
        )
        .expect_err("oversized proposal"),
        "the named-technique timeline proposal is too large"
    );
}

#[test]
fn instruction_step_updates_snapshot_document_dirty_state_and_history() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let fingerprint = project.editor.fold_model_fingerprint_v1();
    let step_id = InstructionStepId::new();
    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::AddInstructionStep {
            step: InstructionStep {
                id: step_id,
                title: "折る前".to_owned(),
                description: "平らな開始姿勢".to_owned(),
                caution: String::new(),
                duration_ms: 1_500,
                visual: Default::default(),
                pose: InstructionPose {
                    model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                    source_model_fingerprint: fingerprint.clone(),
                    fixed_face: None,
                    hinge_angles: Vec::new(),
                },
            },
        },
    )
    .expect("add planar instruction step");

    assert_eq!(response.revision, 1);
    assert!(response.is_dirty);
    assert_eq!(response.fold_model_fingerprint, fingerprint);
    assert_eq!(response.instruction_timeline.steps.len(), 1);
    assert_eq!(response.instruction_timeline.steps[0].id, step_id);
    assert_eq!(
        project.document().instruction_timeline,
        response.instruction_timeline
    );

    let bytes = write_project_ori2(&project.document()).expect("persist instruction timeline");
    let restored = read_project_ori2_with_limits(&bytes, Ori2Limits::default())
        .expect("restore instruction timeline");
    assert_eq!(
        restored.instruction_timeline,
        project.document().instruction_timeline
    );

    project.editor.undo(1).expect("undo instruction addition");
    assert!(project.editor.instruction_timeline().steps.is_empty());
    assert!(!project.is_dirty());
    project.editor.redo(2).expect("redo instruction addition");
    assert_eq!(project.editor.instruction_timeline().steps[0].id, step_id);
    assert!(project.is_dirty());

    let duplicated =
        duplicate_instruction_step_record(project.editor.instruction_timeline(), step_id)
            .expect("duplicate existing instruction step");
    assert_ne!(duplicated.id, step_id);
    let mut expected = project.editor.instruction_timeline().steps[0].clone();
    expected.id = duplicated.id;
    assert_eq!(duplicated, expected);
    project
        .editor
        .execute(
            3,
            Command::AddInstructionStep {
                step: duplicated.clone(),
            },
        )
        .expect("append duplicated instruction atomically");
    assert_eq!(project.editor.instruction_timeline().steps.len(), 2);
    project
        .editor
        .undo(4)
        .expect("undo instruction duplication");
    assert_eq!(project.editor.instruction_timeline().steps.len(), 1);
    project
        .editor
        .redo(5)
        .expect("redo instruction duplication");
    assert_eq!(project.editor.instruction_timeline().steps[1], duplicated);
    let duplicated_archive =
        write_project_ori2(&project.document()).expect("persist duplicated instruction timeline");
    let duplicated_restored =
        read_project_ori2_with_limits(&duplicated_archive, Ori2Limits::default())
            .expect("restore duplicated instruction timeline");
    assert_eq!(
        duplicated_restored.instruction_timeline.steps[1],
        duplicated
    );
    assert_eq!(
        duplicate_instruction_step_record(
            project.editor.instruction_timeline(),
            InstructionStepId::new()
        ),
        Err("instruction_step_not_found".to_owned()),
    );

    let mut certified = project.editor.instruction_timeline().steps[0].clone();
    certified.visual.path_certificate_reference_v1 = Some(ori_domain::PathCertificateReferenceV1 {
        version: 1,
        model_id: ori_domain::PATH_CERTIFICATE_REFERENCE_MODEL_ID_V1.to_owned(),
        binding_sha256: [1; 32],
        source_pose_sha256: [2; 32],
        target_pose_sha256: [3; 32],
        source_model_binding_sha256: [4; 32],
        transition_count: 1,
    });
    certified.visual.cycle_layer_order_proof_v1 = Some(ori_domain::CycleLayerOrderProofV1 {
        version: 1,
        model_id: ori_domain::CYCLE_LAYER_ORDER_PROOF_MODEL_ID_V1.to_owned(),
        target_order_sha256: [5; 32],
        transition_count: 1,
        pairs: Vec::new(),
    });
    certified.visual.named_technique_compiler_v1 =
        Some(ori_domain::NamedTechniqueCompilerMetadataV1 {
            version: 1,
            model_id: ori_domain::NAMED_TECHNIQUE_COMPILER_MODEL_ID_V1.to_owned(),
            technique_kind: "book".to_owned(),
            segment_index: 0,
            segment_count: 1,
            compiler_output_sha256: [6; 32],
        });
    let stripped = duplicate_instruction_step_record(
        &InstructionTimeline {
            steps: vec![certified.clone()],
        },
        certified.id,
    )
    .expect("duplicate strips sequence-bound evidence");
    assert!(certified.visual.path_certificate_reference_v1.is_some());
    assert!(certified.visual.cycle_layer_order_proof_v1.is_some());
    assert!(certified.visual.named_technique_compiler_v1.is_some());
    assert!(stripped.visual.path_certificate_reference_v1.is_none());
    assert!(stripped.visual.cycle_layer_order_proof_v1.is_none());
    assert!(stripped.visual.named_technique_compiler_v1.is_none());
    let mut expected_stripped = certified;
    expected_stripped.id = stripped.id;
    expected_stripped.visual.path_certificate_reference_v1 = None;
    expected_stripped.visual.cycle_layer_order_proof_v1 = None;
    expected_stripped.visual.named_technique_compiler_v1 = None;
    assert_eq!(stripped, expected_stripped);
}

#[test]
fn loaded_current_instruction_poses_are_semantically_checked_but_stale_ones_survive() {
    let project = initial_project_state();
    let mut document = project.document();
    let current_fingerprint = project.editor.fold_model_fingerprint_v1();
    let invalid_current_step = InstructionStep {
        id: InstructionStepId::new(),
        title: "不正な現在姿勢".to_owned(),
        description: String::new(),
        caution: String::new(),
        duration_ms: 1_000,
        visual: Default::default(),
        pose: InstructionPose {
            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
            source_model_fingerprint: current_fingerprint,
            fixed_face: Some(FaceId::new()),
            hinge_angles: Vec::new(),
        },
    };
    document
        .instruction_timeline
        .steps
        .push(invalid_current_step.clone());

    assert_eq!(
        validate_document_instruction_poses(&document)
            .expect_err("current malformed pose must fail semantic loading"),
        "instruction step 1 is invalid: a planar instruction pose must not specify a fixed face"
    );

    document.instruction_timeline.steps[0]
        .pose
        .source_model_fingerprint = "f".repeat(64);
    validate_document_instruction_poses(&document)
        .expect("an old-model pose remains loadable as an editable stale step");
}

#[test]
fn delayed_instruction_pose_cannot_land_after_reopening_the_same_document() {
    let project = initial_project_state();
    let project_id = project.project_id;
    let input =
        capture_topology_input(&project, project_id, 0).expect("capture instruction topology");
    let topology = input.analyze();
    let analyzed = AnalyzedInstructionPose {
        project_instance_id: project.instance_id,
        input,
        topology,
        fixed_face: None,
        hinge_angles: Vec::new(),
    };

    let reopened =
        ProjectState::from_valid_document(project.document(), PathBuf::from("same-project.ori2"));
    assert_eq!(reopened.project_id, project_id);
    assert_eq!(reopened.editor.revision(), 0);
    assert_eq!(reopened.editor.pattern(), project.editor.pattern());
    assert_eq!(reopened.editor.paper(), project.editor.paper());
    assert_ne!(reopened.instance_id, project.instance_id);
    let before = project_state_signature(&reopened);

    assert_eq!(
        super::finish_instruction_pose(&reopened, reopened.instance_id, project_id, 0, analyzed,)
            .expect_err("an old open-instance analysis must not mutate the reopened project"),
        "the open project instance changed while the instruction pose was being analyzed"
    );
    assert_eq!(project_state_signature(&reopened), before);
}

#[test]
fn instruction_pose_capture_rejects_same_document_revision_after_reopen_aba() {
    let project = initial_project_state();
    let stale_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let reopened =
        ProjectState::from_valid_document(project.document(), PathBuf::from("same-project.ori2"));
    assert_eq!(reopened.project_id, expected_project_id);
    assert_eq!(reopened.editor.revision(), expected_revision);
    assert_ne!(reopened.instance_id, stale_instance_id);
    let state = AppState::new(reopened);
    let before = {
        let project = lock_project(&state).expect("lock reopened project");
        project_state_signature(&project)
    };

    let result = tauri::async_runtime::block_on(analyze_instruction_pose(
        &state,
        stale_instance_id,
        expected_project_id,
        expected_revision,
        None,
        Vec::new(),
    ));
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("reopened ABA instance must reject delayed instruction analysis"),
    };

    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    let project = lock_project(&state).expect("lock unchanged reopened project");
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn semantic_instruction_failure_cannot_overwrite_an_existing_save() {
    let project = initial_project_state();
    let mut document = project.document();
    document.instruction_timeline.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: "不正な現在姿勢".to_owned(),
        description: String::new(),
        caution: String::new(),
        duration_ms: 1_000,
        visual: Default::default(),
        pose: InstructionPose {
            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
            source_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
            fixed_face: Some(FaceId::new()),
            hinge_angles: Vec::new(),
        },
    });
    let directory = TestDirectory::new();
    let path = directory.join("existing.ori2");
    let original = b"existing project bytes";
    fs::write(&path, original).expect("create existing target");

    let error = persist_document(&path, &document)
        .expect_err("semantic validation must run before staging a save");

    assert_eq!(error, PROJECT_INSTRUCTIONS_SAVE_FAILED_MESSAGE);
    assert!(!error.contains("不正な現在姿勢"));
    assert_eq!(fs::read(&path).expect("read preserved target"), original);
    assert_eq!(
        fs::read_dir(&directory.path)
            .expect("inspect save directory")
            .count(),
        1,
        "semantic rejection must not leave a staged file"
    );
}

#[test]
fn topology_bridge_preserves_three_faces_and_two_hinges_for_multiple_folds() {
    let project = cellular_multi_fold_project_state();
    let before = project_state_signature(&project);
    let input = capture_topology_input(&project, project.project_id, 0)
        .expect("capture cellular fold graph");

    let response = finish_topology_response(&project, &input, input.analyze())
        .expect("finish cellular fold topology");

    assert!(response.simulation_ready);
    assert!(response.issues.is_empty());
    let snapshot = response.snapshot.expect("cellular fold snapshot");
    assert_eq!(snapshot.source_revision, 0);
    assert_eq!(snapshot.faces.len(), 3);
    assert_eq!(snapshot.hinge_adjacency.len(), 2);
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn topology_bridge_preserves_structured_unsupported_diagnostics() {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("cut-enabled sheet");
    let (pattern, paper) = sheet.into_parts();
    let mut project = ProjectState::new_with_paper(pattern, paper);
    let boundary = project.editor.paper().boundary_vertices.clone();
    let cut = EdgeId::new();
    let project_id = project.project_id;
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddEdge {
            id: cut,
            start: boundary[0],
            end: boundary[2],
            kind: EdgeKind::Cut,
        },
    )
    .expect("add supported editor cut");
    let input = capture_topology_input(&project, project_id, 1).expect("capture unsupported graph");

    let response = finish_topology_response(&project, &input, input.analyze())
        .expect("unsupported topology is a diagnostic response");

    assert!(!response.simulation_ready);
    assert!(response.snapshot.is_none());
    assert!(matches!(
        response.issues.as_slice(),
        [TopologyIssue {
            kind: ori_core::TopologyIssueKind::UnsupportedActiveEdge {
                edge,
                edge_kind: EdgeKind::Cut,
            },
            ..
        }] if *edge == cut
    ));
}

#[test]
fn topology_bridge_rejects_stale_capture_and_delayed_aba_result() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    assert_eq!(
        capture_topology_input(&project, project_id, 1).expect_err("stale requested revision"),
        "expected revision 1, but the current revision is 0"
    );
    assert!(capture_topology_input(&project, ProjectId::new(), 0).is_err());

    let input = capture_topology_input(&project, project_id, 0).expect("capture old input");
    let topology = input.analyze();
    let replacement = create_rectangular_sheet(210.0, 297.0, false).expect("replacement rectangle");
    let (pattern, paper) = replacement.into_parts();
    project.editor = EditorState::with_paper(pattern, paper);
    assert_eq!(project.editor.revision(), 0, "ABA revision fixture");

    assert_eq!(
        finish_topology_response(&project, &input, topology)
            .expect_err("same identity/revision with different content is stale"),
        "the project changed while topology was being analyzed"
    );
}

#[test]
fn validation_worker_releases_project_lock_during_exact_analysis() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let worker_state = Arc::clone(&state);
    let (entered_tx, entered_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);

    let validation = thread::spawn(move || {
        tauri::async_runtime::block_on(validate_project_with_worker(&worker_state, move |input| {
            entered_tx.send(()).expect("announce worker entry");
            release_rx.recv().expect("release validation worker");
            Ok(analyze_validation_input(input))
        }))
    });

    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("validation worker must start");
    let lock_was_available = state.0.try_lock().is_ok();
    release_tx.send(()).expect("release validation worker");

    let snapshot = validation
        .join()
        .expect("validation caller thread must not panic")
        .expect("unchanged validation must finish");
    assert!(
        lock_was_available,
        "the project mutex must not be held during exact validation"
    );
    assert_eq!(snapshot.revision, 0);
}

#[test]
fn validation_worker_rejects_same_revision_aba_content() {
    let state = Arc::new(AppState::new(initial_project_state()));
    let worker_state = Arc::clone(&state);
    let (entered_tx, entered_rx) = mpsc::sync_channel(0);
    let (release_tx, release_rx) = mpsc::sync_channel(0);

    let validation = thread::spawn(move || {
        tauri::async_runtime::block_on(validate_project_with_worker(&worker_state, move |input| {
            entered_tx.send(()).expect("announce worker entry");
            release_rx.recv().expect("release validation worker");
            Ok(analyze_validation_input(input))
        }))
    });

    entered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("validation worker must start");
    {
        let Ok(mut project) = state.0.try_lock() else {
            release_tx
                .send(())
                .expect("release blocked validation worker");
            validation
                .join()
                .expect("validation caller thread must not panic")
                .expect("unchanged validation must finish");
            panic!("the project mutex must be available while validation is running");
        };
        let replacement =
            create_rectangular_sheet(210.0, 297.0, false).expect("replacement rectangle");
        let (pattern, paper) = replacement.into_parts();
        project.editor = EditorState::with_paper(pattern, paper);
        assert_eq!(project.editor.revision(), 0, "ABA revision fixture");
    }
    release_tx.send(()).expect("release validation worker");

    let error = validation
        .join()
        .expect("validation caller thread must not panic")
        .expect_err("same-revision replacement must make the result stale");
    assert_eq!(
        error,
        "the project changed while validation was being analyzed"
    );
}

#[test]
fn validation_worker_panic_and_reported_failure_are_redacted_and_fail_closed() {
    let state = AppState::new(initial_project_state());
    let private_panic = r"C:\Users\alice\秘密の作品.ori2 at vertex=(12.3,45.6)";

    let panic_error = tauri::async_runtime::block_on(validate_project_with_worker(
        &state,
        move |_| -> Result<AnalyzedProjectValidation, String> {
            panic!("{private_panic}");
        },
    ))
    .expect_err("a panicking worker must fail the command");
    assert_eq!(panic_error, VALIDATION_ANALYSIS_FAILED_MESSAGE);
    assert!(!panic_error.contains("alice"));
    assert!(!panic_error.contains("秘密の作品"));
    assert!(!panic_error.contains("12.3"));

    let private_failure = r"C:\Users\bob\非公開.ori2; internal_id=validation-7";
    let reported_error =
        tauri::async_runtime::block_on(validate_project_with_worker(&state, move |_| {
            Err(private_failure.to_owned())
        }))
        .expect_err("a reported worker failure must fail the command");
    assert_eq!(reported_error, VALIDATION_ANALYSIS_FAILED_MESSAGE);
    assert!(!reported_error.contains("bob"));
    assert!(!reported_error.contains("非公開"));
    assert!(!reported_error.contains("validation-7"));
    assert!(
        state.0.try_lock().is_ok(),
        "worker failures must not poison or retain the project mutex"
    );
}

#[test]
fn background_task_failures_discard_private_panic_payloads() {
    let private_payload = r"C:\Users\alice\秘密の作品.ori2; face_id=private; point=(12.3,45.6)";
    let errors = [
        topology_analysis_task_error(private_payload),
        instruction_topology_analysis_task_error(private_payload),
        fold_import_task_error(private_payload),
        fold_conversion_task_error(private_payload),
    ];

    assert_eq!(
        errors,
        [
            TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE,
            INSTRUCTION_TOPOLOGY_ANALYSIS_TASK_FAILED_MESSAGE,
            FOLD_IMPORT_TASK_FAILED_MESSAGE,
            FOLD_CONVERSION_TASK_FAILED_MESSAGE,
        ]
    );
    for error in errors {
        assert!(!error.contains("alice"));
        assert!(!error.contains("秘密の作品"));
        assert!(!error.contains("face_id"));
        assert!(!error.contains("12.3"));
    }
}

fn unsaved_project_with_redo_history(name: &str) -> ProjectState {
    let mut project =
        ProjectState::new_unsaved(name.to_owned(), CreasePattern::empty(), Paper::default());
    let project_id = project.project_id;
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(12.0, 34.0),
        },
    )
    .expect("add history fixture vertex");
    project.editor.undo(1).expect("create redo history");
    assert!(project.editor.can_redo());
    project
}

fn unsaved_project_with_undo_and_redo_history(name: &str) -> (ProjectState, VertexId, VertexId) {
    let mut project =
        ProjectState::new_unsaved(name.to_owned(), CreasePattern::empty(), Paper::default());
    project
        .editor
        .set_history_entry_limit(17)
        .expect("configure persisted history limit");
    let project_id = project.project_id;
    let first = VertexId::new();
    let second = VertexId::new();
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddVertex {
            id: first,
            position: Point2::new(12.0, 34.0),
        },
    )
    .expect("add first history fixture vertex");
    execute_command(
        &mut project,
        project_id,
        1,
        Command::AddVertex {
            id: second,
            position: Point2::new(56.0, 78.0),
        },
    )
    .expect("add second history fixture vertex");
    project
        .editor
        .undo(2)
        .expect("leave both Undo and Redo stacks populated");
    assert!(project.editor.can_undo());
    assert!(project.editor.can_redo());
    (project, first, second)
}

fn project_with_reachable_invalid_instruction_pose(name: &str) -> ProjectState {
    let sheet = create_rectangular_sheet(40.0, 40.0, false).expect("valid history test sheet");
    let (pattern, paper) = sheet.into_parts();
    let mut project = ProjectState::new_unsaved(name.to_owned(), pattern, paper);
    let project_id = project.project_id;
    let old_fingerprint = project.editor.fold_model_fingerprint_v1();
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddInstructionStep {
            step: InstructionStep {
                id: InstructionStepId::new(),
                title: "invalid only after Undo".to_owned(),
                description: String::new(),
                caution: String::new(),
                duration_ms: 1_000,
                visual: Default::default(),
                pose: InstructionPose {
                    model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                    source_model_fingerprint: old_fingerprint.clone(),
                    fixed_face: Some(FaceId::new()),
                    hinge_angles: Vec::new(),
                },
            },
        },
    )
    .expect("the editor accepts structurally valid pose metadata");
    execute_command(
        &mut project,
        project_id,
        1,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(20.0, 20.0),
        },
    )
    .expect("make the invalid instruction pose stale in the current document");
    assert_ne!(project.editor.fold_model_fingerprint_v1(), old_fingerprint);
    assert!(
        validate_document_instruction_poses(&project.document()).is_ok(),
        "the final stale pose is intentionally accepted"
    );
    let mut undo_endpoint = project.editor.clone();
    undo_endpoint.undo(2).expect("reach old model endpoint");
    let mut endpoint_document = project.document();
    endpoint_document.paper = undo_endpoint.paper().clone();
    endpoint_document.crease_pattern = undo_endpoint.pattern().clone();
    endpoint_document.instruction_timeline = undo_endpoint.instruction_timeline().clone();
    endpoint_document.geometric_constraints = undo_endpoint.geometric_constraints().clone();
    endpoint_document.layers = undo_endpoint.project_layers().clone();
    assert!(
        validate_document_instruction_poses(&endpoint_document).is_err(),
        "the same pose becomes current and invalid after Undo"
    );
    project
}

fn project_with_redo_reachable_invalid_instruction_pose(name: &str) -> ProjectState {
    let sheet = create_rectangular_sheet(40.0, 40.0, false).expect("valid history test sheet");
    let (pattern, paper) = sheet.into_parts();
    let mut project = ProjectState::new_unsaved(name.to_owned(), pattern, paper);
    let project_id = project.project_id;
    let fingerprint = project.editor.fold_model_fingerprint_v1();
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddInstructionStep {
            step: InstructionStep {
                id: InstructionStepId::new(),
                title: "invalid only after Redo".to_owned(),
                description: String::new(),
                caution: String::new(),
                duration_ms: 1_000,
                visual: Default::default(),
                pose: InstructionPose {
                    model: InstructionPoseModel::AbsoluteHingeAnglesV1,
                    source_model_fingerprint: fingerprint,
                    fixed_face: Some(FaceId::new()),
                    hinge_angles: Vec::new(),
                },
            },
        },
    )
    .expect("the editor accepts structurally valid pose metadata");
    project
        .editor
        .undo(1)
        .expect("leave the invalid step only on the Redo endpoint");
    assert!(project.editor.instruction_timeline().steps.is_empty());
    assert!(project.editor.can_redo());
    assert!(validate_document_instruction_poses(&project.document()).is_ok());
    project
}

fn corrupt_editor_history_payload(mut bytes: Vec<u8>) -> Vec<u8> {
    const LOCAL_FILE_HEADER_SIZE: usize = 30;
    const HISTORY_PATH: &[u8] = b"editor-history.json";
    let name_start = bytes
        .windows(HISTORY_PATH.len())
        .position(|window| window == HISTORY_PATH)
        .expect("history local-header name");
    let header_start = name_start
        .checked_sub(LOCAL_FILE_HEADER_SIZE)
        .expect("history local-header offset");
    assert_eq!(
        &bytes[header_start..header_start + 4],
        b"PK\x03\x04",
        "the first history path must belong to its local ZIP header"
    );
    let compressed_size = u32::from_le_bytes(
        bytes[header_start + 18..header_start + 22]
            .try_into()
            .expect("compressed-size field"),
    ) as usize;
    let extra_length = u16::from_le_bytes(
        bytes[header_start + 28..header_start + 30]
            .try_into()
            .expect("extra-length field"),
    ) as usize;
    assert!(compressed_size > 0);
    let payload_start = name_start + HISTORY_PATH.len() + extra_length;
    let corrupt_at = payload_start + compressed_size / 2;
    bytes[corrupt_at] ^= 0x01;
    bytes
}

fn tamper_ori2_project_certificate(bytes: &[u8], reauthenticate_manifest: bool) -> Vec<u8> {
    let mut source = ZipArchive::new(Cursor::new(bytes)).expect("open source ORI2");
    let mut entries = Vec::with_capacity(source.len());
    for index in 0..source.len() {
        let mut entry = source.by_index(index).expect("read source ORI2 entry");
        let name = entry.name().to_owned();
        let mut payload = Vec::new();
        entry.read_to_end(&mut payload).expect("read ORI2 payload");
        entries.push((name, entry.compression(), payload));
    }
    let project_payload = &mut entries
        .iter_mut()
        .find(|(name, _, _)| name == ori_formats::PROJECT_FOLDER_PROJECT_PATH)
        .expect("ORI2 project entry")
        .2;
    let mut project: serde_json::Value = serde_json::from_slice(project_payload).unwrap();
    let certificate_byte = project
        .pointer_mut(
            "/beginner_design_profile/generation_provenance/fold_path_certificate_sha256/0",
        )
        .expect("generic tree certificate byte");
    *certificate_byte = serde_json::json!(certificate_byte.as_u64().unwrap_or_default() ^ 1);
    *project_payload = serde_json::to_vec(&project).unwrap();
    if reauthenticate_manifest {
        let project_size = project_payload.len() as u64;
        let project_sha256 = sha2::Sha256::digest(project_payload.as_slice())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let manifest_payload = &mut entries
            .iter_mut()
            .find(|(name, _, _)| name == ori_formats::PROJECT_FOLDER_MANIFEST_PATH)
            .expect("ORI2 manifest entry")
            .2;
        let mut manifest: serde_json::Value = serde_json::from_slice(manifest_payload).unwrap();
        let descriptor = manifest
            .get_mut("project")
            .expect("ORI2 project descriptor");
        descriptor["uncompressed_size"] = serde_json::json!(project_size);
        descriptor["sha256"] = serde_json::json!(project_sha256);
        *manifest_payload = serde_json::to_vec(&manifest).unwrap();
    }
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, compression, payload) in entries {
        writer
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(compression),
            )
            .expect("start tampered ORI2 entry");
        writer
            .write_all(&payload)
            .expect("write tampered ORI2 entry");
    }
    writer.finish().expect("finish tampered ORI2").into_inner()
}

fn file_document(name: &str, x: f64) -> ProjectDocument {
    ProjectDocument::new(
        name,
        CreasePattern {
            vertices: vec![Vertex {
                id: VertexId::new(),
                position: Point2::new(x, 5.0),
            }],
            edges: Vec::new(),
        },
    )
}

fn crossing_project() -> (ProjectState, Edge, Edge) {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let ids = [
        VertexId::new(),
        VertexId::new(),
        VertexId::new(),
        VertexId::new(),
    ];
    pattern.vertices.extend([
        Vertex {
            id: ids[0],
            position: Point2::new(20.0, 20.0),
        },
        Vertex {
            id: ids[1],
            position: Point2::new(80.0, 80.0),
        },
        Vertex {
            id: ids[2],
            position: Point2::new(20.0, 80.0),
        },
        Vertex {
            id: ids[3],
            position: Point2::new(80.0, 20.0),
        },
    ]);
    let first = Edge {
        id: EdgeId::new(),
        start: ids[0],
        end: ids[1],
        kind: EdgeKind::Mountain,
    };
    let second = Edge {
        id: EdgeId::new(),
        start: ids[2],
        end: ids[3],
        kind: EdgeKind::Valley,
    };
    pattern.edges.extend([first.clone(), second.clone()]);
    (ProjectState::new_with_paper(pattern, paper), first, second)
}

fn t_junction_project() -> (ProjectState, Edge, Edge, VertexId) {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let interior_start = VertexId::new();
    let interior_end = VertexId::new();
    let stem_other = VertexId::new();
    let junction = VertexId::new();
    pattern.vertices.extend([
        Vertex {
            id: interior_start,
            position: Point2::new(10.0, 40.0),
        },
        Vertex {
            id: interior_end,
            position: Point2::new(90.0, 40.0),
        },
        Vertex {
            id: stem_other,
            position: Point2::new(34.0, 10.0),
        },
        Vertex {
            id: junction,
            position: Point2::new(34.0, 40.0),
        },
    ]);
    let interior = Edge {
        id: EdgeId::new(),
        start: interior_start,
        end: interior_end,
        kind: EdgeKind::Mountain,
    };
    let stem = Edge {
        id: EdgeId::new(),
        start: stem_other,
        end: junction,
        kind: EdgeKind::Valley,
    };
    pattern.edges.extend([interior.clone(), stem.clone()]);
    (
        ProjectState::new_with_paper(pattern, paper),
        interior,
        stem,
        junction,
    )
}

fn boundary_t_junction_project() -> (ProjectState, Edge, Edge, VertexId) {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let boundary = pattern.edges[0].clone();
    let junction = VertexId::new();
    let stem_other = VertexId::new();
    pattern.vertices.extend([
        Vertex {
            id: junction,
            position: Point2::new(40.0, 0.0),
        },
        Vertex {
            id: stem_other,
            position: Point2::new(40.0, 30.0),
        },
    ]);
    let stem = Edge {
        id: EdgeId::new(),
        start: stem_other,
        end: junction,
        kind: EdgeKind::Mountain,
    };
    pattern.edges.push(stem.clone());
    (
        ProjectState::new_with_paper(pattern, paper),
        boundary,
        stem,
        junction,
    )
}

fn append_cluster_test_edge(
    pattern: &mut CreasePattern,
    start_position: Point2,
    end_position: Point2,
    kind: EdgeKind,
) -> Edge {
    let start = VertexId::new();
    let end = VertexId::new();
    pattern.vertices.extend([
        Vertex {
            id: start,
            position: start_position,
        },
        Vertex {
            id: end,
            position: end_position,
        },
    ]);
    let edge = Edge {
        id: EdgeId::new(),
        start,
        end,
        kind,
    };
    pattern.edges.push(edge.clone());
    edge
}

fn create_cluster_project(include_omitted_edge: bool) -> (ProjectState, Vec<Edge>) {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let mut edges = vec![
        append_cluster_test_edge(
            &mut pattern,
            Point2::new(10.0, 50.0),
            Point2::new(90.0, 50.0),
            EdgeKind::Mountain,
        ),
        append_cluster_test_edge(
            &mut pattern,
            Point2::new(50.0, 10.0),
            Point2::new(50.0, 90.0),
            EdgeKind::Valley,
        ),
        append_cluster_test_edge(
            &mut pattern,
            Point2::new(20.0, 20.0),
            Point2::new(80.0, 80.0),
            EdgeKind::Auxiliary,
        ),
    ];
    if include_omitted_edge {
        edges.push(append_cluster_test_edge(
            &mut pattern,
            Point2::new(20.0, 80.0),
            Point2::new(80.0, 20.0),
            EdgeKind::Mountain,
        ));
    }
    (ProjectState::new_with_paper(pattern, paper), edges)
}

fn maximum_cluster_project() -> (ProjectState, Vec<Edge>) {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let mut edges = Vec::with_capacity(MAX_INTERSECTION_CLUSTER_TARGETS);
    for index in 0..MAX_INTERSECTION_CLUSTER_TARGETS {
        let offset = index as f64 - 32.0;
        let edge = append_cluster_test_edge(
            &mut pattern,
            Point2::new(10.0, 50.0 - offset),
            Point2::new(90.0, 50.0 + offset),
            match index % 4 {
                0 => EdgeKind::Mountain,
                1 => EdgeKind::Valley,
                2 => EdgeKind::Auxiliary,
                _ => EdgeKind::Cut,
            },
        );
        edges.push(edge);
    }
    (ProjectState::new_with_paper(pattern, paper), edges)
}

fn reuse_cluster_project() -> (ProjectState, [Edge; 3], VertexId) {
    let sheet = create_rectangular_sheet(100.0, 100.0, true).expect("valid test sheet");
    let (mut pattern, paper) = sheet.into_parts();
    let horizontal = append_cluster_test_edge(
        &mut pattern,
        Point2::new(10.0, 50.0),
        Point2::new(90.0, 50.0),
        EdgeKind::Mountain,
    );
    let vertical = append_cluster_test_edge(
        &mut pattern,
        Point2::new(50.0, 10.0),
        Point2::new(50.0, 90.0),
        EdgeKind::Valley,
    );
    let junction = VertexId::new();
    let stem_start = VertexId::new();
    pattern.vertices.extend([
        Vertex {
            id: stem_start,
            position: Point2::new(20.0, 20.0),
        },
        Vertex {
            id: junction,
            position: Point2::new(50.0, 50.0),
        },
    ]);
    let stem = Edge {
        id: EdgeId::new(),
        start: stem_start,
        end: junction,
        kind: EdgeKind::Auxiliary,
    };
    pattern.edges.push(stem.clone());
    (
        ProjectState::new_with_paper(pattern, paper),
        [horizontal, vertical, stem],
        junction,
    )
}

#[test]
fn benchmark_pattern_response_contains_stable_renderable_geometry() {
    let response = generate_benchmark_pattern(4);

    assert_eq!(response.requested_edge_count, 4);
    assert_eq!(response.vertex_count, 4);
    assert_eq!(response.edge_count, 4);
    assert_eq!(
        response.vertices,
        vec![
            BenchmarkVertex {
                id: "benchmark-v-0".to_owned(),
                position: Point2::new(0.0, 0.0),
            },
            BenchmarkVertex {
                id: "benchmark-v-1".to_owned(),
                position: Point2::new(1.0, 0.0),
            },
            BenchmarkVertex {
                id: "benchmark-v-2".to_owned(),
                position: Point2::new(0.0, 1.0),
            },
            BenchmarkVertex {
                id: "benchmark-v-3".to_owned(),
                position: Point2::new(1.0, 1.0),
            },
        ]
    );
    assert_eq!(
        response.edges,
        vec![
            BenchmarkEdge {
                id: "benchmark-e-0".to_owned(),
                start: "benchmark-v-0".to_owned(),
                end: "benchmark-v-1".to_owned(),
                kind: EdgeKind::Mountain,
            },
            BenchmarkEdge {
                id: "benchmark-e-1".to_owned(),
                start: "benchmark-v-0".to_owned(),
                end: "benchmark-v-2".to_owned(),
                kind: EdgeKind::Valley,
            },
            BenchmarkEdge {
                id: "benchmark-e-2".to_owned(),
                start: "benchmark-v-1".to_owned(),
                end: "benchmark-v-3".to_owned(),
                kind: EdgeKind::Mountain,
            },
            BenchmarkEdge {
                id: "benchmark-e-3".to_owned(),
                start: "benchmark-v-2".to_owned(),
                end: "benchmark-v-3".to_owned(),
                kind: EdgeKind::Valley,
            },
        ]
    );
    assert_eq!(generate_benchmark_pattern(4), response);
}

#[test]
fn benchmark_pattern_response_has_all_ten_thousand_edges_and_valid_references() {
    let response = generate_benchmark_pattern(10_000);

    assert_eq!(response.requested_edge_count, 10_000);
    assert_eq!(response.vertex_count, 5_184);
    assert_eq!(response.edge_count, 10_000);
    let vertex_ids = response
        .vertices
        .iter()
        .map(|vertex| vertex.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert!(response.edges.iter().all(|edge| {
        vertex_ids.contains(edge.start.as_str()) && vertex_ids.contains(edge.end.as_str())
    }));
}

#[test]
fn benchmark_pattern_response_is_empty_for_zero_edges() {
    let response = generate_benchmark_pattern(0);

    assert_eq!(response.requested_edge_count, 0);
    assert_eq!(response.vertex_count, 0);
    assert_eq!(response.edge_count, 0);
    assert!(response.vertices.is_empty());
    assert!(response.edges.is_empty());
}

#[test]
fn project_name_is_trimmed_and_validated_by_unicode_character_count() {
    assert_eq!(normalize_project_name("  Crane  "), Ok("Crane".to_owned()));
    assert_eq!(
        normalize_project_name("\n  Crane  \t"),
        Ok("Crane".to_owned())
    );
    assert!(normalize_project_name("").is_err());
    assert!(normalize_project_name(" \t\n ").is_err());
    assert!(normalize_project_name("Crane\0draft").is_err());

    let maximum = "鶴".repeat(MAX_PROJECT_NAME_CHARS);
    assert_eq!(normalize_project_name(&maximum), Ok(maximum.clone()));
    assert!(normalize_project_name(&format!("{maximum}鶴")).is_err());
}

#[test]
fn paper_thickness_accepts_zero_and_rejects_negative_or_non_finite_values() {
    assert_eq!(validate_paper_thickness(0.0), Ok(()));
    assert_eq!(validate_paper_thickness(-0.0), Ok(()));
    for invalid in [-f64::MIN_POSITIVE, -1.0, f64::NAN, f64::INFINITY] {
        assert!(validate_paper_thickness(invalid).is_err());
    }
}

#[test]
fn new_project_state_has_requested_paper_and_no_saved_baseline() {
    let parameters = new_project_parameters();
    let expected_front = parameters.front_color;
    let expected_back = parameters.back_color;

    let project = create_new_project_state(parameters).expect("valid new project");
    let response = snapshot(&project);

    assert_eq!(project.name, "Test sheet");
    assert!(project.current_path.is_none());
    assert!(project.saved_revision.is_none());
    assert!(project.saved_document.is_none());
    assert_eq!(project.editor.revision(), 0);
    assert!(!project.editor.can_undo());
    assert!(!project.editor.can_redo());
    assert!(project.editor.cutting_allowed());
    assert!(project.is_dirty());
    assert_eq!(project.editor.paper().thickness_mm, 0.2);
    assert_eq!(project.editor.paper().front.color, expected_front);
    assert_eq!(project.editor.paper().back.color, expected_back);
    assert_eq!(project.editor.paper().front.texture_asset, None);
    assert_eq!(project.editor.paper().back.texture_asset, None);
    let creation_expressions = project
        .numeric_expressions
        .rectangular_paper_creation
        .as_ref()
        .expect("new project keeps both creation expressions");
    assert_eq!(creation_expressions.schema_version, 1);
    assert_eq!(creation_expressions.width_source, "210");
    assert_eq!(creation_expressions.height_source, "297");
    assert_eq!(creation_expressions.adopted_width_mm, 210.0);
    assert_eq!(creation_expressions.adopted_height_mm, 297.0);
    assert_eq!(
        response.numeric_expressions, project.numeric_expressions,
        "snapshot and persisted document share the same bounded metadata"
    );
    assert_eq!(
        project.document().numeric_expressions,
        project.numeric_expressions
    );
    assert_eq!(
        project.editor.pattern().vertices[2].position,
        Point2::new(210.0, 297.0)
    );
    assert!(validate_paper(project.editor.paper(), project.editor.pattern()).is_valid());

    assert_eq!(response.project_id, project.project_id);
    assert_eq!(response.name, "Test sheet");
    assert!(response.current_path.is_none());
    assert_eq!(response.revision, 0);
    assert!(response.saved_revision.is_none());
    assert!(response.is_dirty);
    assert_eq!(&response.paper, project.editor.paper());
    assert!(response.cutting_allowed);
    assert!(!response.can_undo);
    assert!(!response.can_redo);
}

#[test]
fn loaded_numeric_expressions_are_re_evaluated_against_saved_adopted_values() {
    assert_eq!(
        map_loaded_numeric_expression_error(PositiveMillimetrePairError::WorkerBusy),
        PROJECT_NUMERIC_EXPRESSIONS_BUSY_MESSAGE
    );
    let project = create_new_project_state(new_project_parameters()).expect("valid new project");
    let document = project.document();
    validate_loaded_numeric_expression_bindings(&document)
        .expect("untampered expressions remain loadable");

    let mut changed_source = document.clone();
    changed_source
        .numeric_expressions
        .rectangular_paper_creation
        .as_mut()
        .expect("creation expressions")
        .width_source = "211".to_owned();
    assert_eq!(
        validate_loaded_numeric_expression_bindings(&changed_source),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );

    let mut changed_value = document.clone();
    changed_value
        .numeric_expressions
        .rectangular_paper_creation
        .as_mut()
        .expect("creation expressions")
        .adopted_height_mm = 298.0;
    assert_eq!(
        validate_loaded_numeric_expression_bindings(&changed_value),
        Err(PROJECT_NUMERIC_EXPRESSIONS_INVALID_MESSAGE.to_owned())
    );

    let mut legacy = document;
    legacy.numeric_expressions = ProjectNumericExpressions::default();
    validate_loaded_numeric_expression_bindings(&legacy)
        .expect("legacy projects without expressions migrate safely");
}

#[test]
fn vertex_coordinate_expressions_follow_native_history_and_archive_round_trip() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let vertex = VertexId::new();
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddVertex {
            id: vertex,
            position: Point2::new(0.5, -2.0),
        },
    )
    .expect("add expression-backed vertex");
    project.adopt_vertex_coordinate_expression(VertexCoordinateExpressions::new(
        vertex, "1 / 2", "-sqrt(4)", 0.5, -2.0,
    ));
    let binding = project.numeric_expressions.vertex_coordinates[0].clone();
    assert_eq!(binding.x_source, "1 / 2");
    assert_eq!(binding.y_source, "-sqrt(4)");
    validate_loaded_numeric_expression_bindings(
        &project
            .project_archive()
            .expect("serialize expression history")
            .document,
    )
    .expect("re-evaluate every persisted expression");

    execute_undo(&mut project, project_id, 1).expect("undo vertex");
    assert!(project.numeric_expressions.vertex_coordinates.is_empty());
    execute_redo(&mut project, project_id, 2).expect("redo vertex");
    assert_eq!(
        project.numeric_expressions.vertex_coordinates,
        vec![binding]
    );
}

#[test]
fn creation_expressions_follow_document_dirty_state_without_entering_editor_undo_history() {
    let mut project =
        create_new_project_state(new_project_parameters()).expect("valid new project");
    let project_id = project.project_id;
    let saved_document = project.document();
    let saved_expressions = project.numeric_expressions.clone();
    project.saved_document = Some(saved_document.clone());
    project.saved_revision = Some(project.editor.revision());
    assert!(!project.is_dirty());

    let resized = execute_command(
        &mut project,
        project_id,
        0,
        Command::ResizeRectangularPaper {
            width_mm: 420.0,
            height_mm: 594.0,
        },
    )
    .expect("resize paper");
    assert!(resized.is_dirty);
    assert_eq!(
        project.numeric_expressions.rectangular_paper_creation,
        saved_expressions.rectangular_paper_creation
    );

    project.editor.undo(1).expect("undo resize");
    assert_eq!(project.document(), saved_document);
    assert_eq!(
        project.numeric_expressions.rectangular_paper_creation,
        saved_expressions.rectangular_paper_creation
    );
    assert!(!project.is_dirty());

    project
        .numeric_expressions
        .rectangular_paper_creation
        .as_mut()
        .expect("creation expressions")
        .width_source = "210 + 0".to_owned();
    assert!(project.is_dirty());
}

#[test]
fn snapshot_paper_uses_the_current_editor_cutting_setting() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    assert!(!project.editor.paper().cutting_allowed);

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::SetCuttingAllowed { allowed: true },
    )
    .expect("enable cutting");

    assert!(response.cutting_allowed);
    assert!(response.paper.cutting_allowed);
    assert!(project.document().paper.cutting_allowed);
}

#[test]
fn paper_properties_follow_undo_redo_dirty_save_and_validation() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let original = project.editor.paper().clone();
    let front_color = RgbaColor::opaque(15, 35, 55);
    let back_color = RgbaColor::opaque(205, 185, 165);

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::UpdatePaperProperties {
            thickness_mm: 0.0,
            front_color,
            back_color,
            front_texture_asset: None,
            back_texture_asset: None,
            cutting_allowed: true,
        },
    )
    .expect("update paper properties");

    assert_eq!(response.revision, 1);
    assert!(response.is_dirty);
    assert_eq!(response.paper.thickness_mm, 0.0);
    assert_eq!(response.paper.front.color, front_color);
    assert_eq!(response.paper.back.color, back_color);
    assert!(response.paper.cutting_allowed);
    assert!(validation_snapshot(&project).is_valid);

    project.editor.undo(1).expect("undo properties");
    assert_eq!(project.editor.paper(), &original);
    assert!(!project.is_dirty());

    project.editor.redo(2).expect("redo properties");
    assert!(project.is_dirty());
    let saved_document = project.document();
    project.saved_revision = Some(project.editor.revision());
    project.saved_document = Some(saved_document.clone());
    assert!(!project.is_dirty());
    assert_eq!(project.document(), saved_document);

    project.editor.undo(3).expect("undo after save");
    assert!(project.is_dirty());
    project.editor.redo(4).expect("redo to saved content");
    assert!(!project.is_dirty());
}

#[test]
fn imported_front_textures_remain_live_across_undo_redo() {
    let mut project = initial_project_state();
    let instance_id = project.instance_id;
    let project_id = project.project_id;
    let png = |tag| {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.push(tag);
        bytes
    };

    register_front_texture(
        &mut project,
        instance_id,
        project_id,
        0,
        ProjectTextureMediaTypeV1::Png,
        png(1),
    )
    .expect("first texture");
    let first = project.editor.paper().front.texture_asset.unwrap();
    register_front_texture(
        &mut project,
        instance_id,
        project_id,
        1,
        ProjectTextureMediaTypeV1::Png,
        png(2),
    )
    .expect("replacement texture");
    let second = project.editor.paper().front.texture_asset.unwrap();
    assert_ne!(first, second);
    assert_eq!(project.texture_assets.len(), 2);

    project.editor.undo(2).expect("undo texture replacement");
    assert_eq!(project.editor.paper().front.texture_asset, Some(first));
    ori_formats::write_project_json(&project.document()).expect("undo document");
    project.editor.redo(3).expect("redo texture replacement");
    assert_eq!(project.editor.paper().front.texture_asset, Some(second));
    ori_formats::write_project_json(&project.document()).expect("redo document");
}

#[test]
fn imported_back_textures_remain_live_across_undo_redo() {
    let mut project = initial_project_state();
    let instance_id = project.instance_id;
    let project_id = project.project_id;
    let png = |tag| {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.push(tag);
        bytes
    };
    register_back_texture(
        &mut project,
        instance_id,
        project_id,
        0,
        ProjectTextureMediaTypeV1::Png,
        png(1),
    )
    .expect("first back texture");
    let first = project.editor.paper().back.texture_asset.unwrap();
    register_back_texture(
        &mut project,
        instance_id,
        project_id,
        1,
        ProjectTextureMediaTypeV1::Png,
        png(2),
    )
    .expect("replacement back texture");
    let second = project.editor.paper().back.texture_asset.unwrap();
    assert_ne!(first, second);
    project.editor.undo(2).expect("undo back texture");
    assert_eq!(project.editor.paper().back.texture_asset, Some(first));
    ori_formats::write_project_json(&project.document()).expect("undo document");
    project.editor.redo(3).expect("redo back texture");
    assert_eq!(project.editor.paper().back.texture_asset, Some(second));
    ori_formats::write_project_json(&project.document()).expect("redo document");
}

#[test]
fn length_display_unit_follows_snapshot_dirty_history_and_fingerprint_contracts() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let original_document = project.document();
    let fingerprint = project.editor.fold_model_fingerprint_v1();
    let reference_edge = project.editor.pattern().edges[0].id;

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::SetLengthDisplayUnit {
            unit: LengthDisplayUnit::PaperEdgeRatio { reference_edge },
        },
    )
    .expect("set native length display unit");

    assert_eq!(response.revision, 1);
    assert!(response.is_dirty);
    assert!(response.can_undo);
    assert!(!response.can_redo);
    assert_eq!(
        response.paper.length_display_unit,
        LengthDisplayUnit::PaperEdgeRatio { reference_edge }
    );
    assert_eq!(response.fold_model_fingerprint, fingerprint);
    assert_eq!(project.editor.fold_model_fingerprint_v1(), fingerprint);
    assert_eq!(
        project.document().paper.length_display_unit,
        LengthDisplayUnit::PaperEdgeRatio { reference_edge }
    );

    project.editor.undo(1).expect("undo display unit");
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());
    assert_eq!(project.editor.fold_model_fingerprint_v1(), fingerprint);

    project.editor.redo(2).expect("redo display unit");
    assert!(project.is_dirty());
    assert_eq!(
        project.editor.paper().length_display_unit,
        LengthDisplayUnit::PaperEdgeRatio { reference_edge }
    );
    assert_eq!(project.editor.fold_model_fingerprint_v1(), fingerprint);
}

#[test]
fn invalid_paper_property_command_preserves_project_state() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    let conflict = execute_command(
        &mut project,
        project_id,
        1,
        Command::UpdatePaperProperties {
            thickness_mm: 0.3,
            front_color: RgbaColor::opaque(1, 2, 3),
            back_color: RgbaColor::opaque(4, 5, 6),
            front_texture_asset: None,
            back_texture_asset: None,
            cutting_allowed: true,
        },
    )
    .expect_err("stale property update must fail");
    assert_eq!(
        conflict,
        "expected revision 1, but the current revision is 0"
    );
    assert_eq!(project_state_signature(&project), before);

    let error = execute_command(
        &mut project,
        project_id,
        0,
        Command::UpdatePaperProperties {
            thickness_mm: f64::NAN,
            front_color: RgbaColor::opaque(1, 2, 3),
            back_color: RgbaColor::opaque(4, 5, 6),
            front_texture_asset: None,
            back_texture_asset: None,
            cutting_allowed: true,
        },
    )
    .expect_err("invalid thickness must fail");

    assert_eq!(error, "paper thickness must be finite");
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn rectangular_resize_updates_document_dirty_state_and_undo_redo() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let original_document = project.document();
    let original_vertex_ids = project
        .editor
        .pattern()
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let original_edges = project.editor.pattern().edges.clone();
    let original_paper = project.editor.paper().clone();

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::ResizeRectangularPaper {
            width_mm: 210.0,
            height_mm: 297.0,
        },
    )
    .expect("resize paper");

    assert_eq!(response.revision, 1);
    assert!(response.is_dirty);
    assert!(response.can_undo);
    assert!(!response.can_redo);
    assert_eq!(response.paper, original_paper);
    assert_eq!(
        response
            .crease_pattern
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>(),
        original_vertex_ids
    );
    assert_eq!(response.crease_pattern.edges, original_edges);
    assert!(
        response
            .crease_pattern
            .vertices
            .iter()
            .any(|vertex| vertex.position == Point2::new(210.0, 297.0))
    );
    assert!(validation_snapshot(&project).is_valid);
    let resized_document = project.document();
    assert_ne!(resized_document, original_document);
    assert_eq!(resized_document.paper, original_paper);

    project.editor.undo(1).expect("undo resize");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project.editor.redo(2).expect("redo resize");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), resized_document);
    assert!(project.is_dirty());
}

#[test]
fn same_size_resize_has_history_without_making_the_document_dirty() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let original_document = project.document();

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::ResizeRectangularPaper {
            width_mm: DEFAULT_SHEET_SIZE_MM,
            height_mm: DEFAULT_SHEET_SIZE_MM,
        },
    )
    .expect("same-size resize");

    assert_eq!(response.revision, 1);
    assert!(response.can_undo);
    assert!(!response.is_dirty);
    assert_eq!(project.document(), original_document);
}

#[test]
fn resize_conflicts_invalid_dimensions_and_overflow_preserve_project_state() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    let conflict = execute_command(
        &mut project,
        project_id,
        1,
        Command::ResizeRectangularPaper {
            width_mm: 210.0,
            height_mm: 297.0,
        },
    )
    .expect_err("stale resize must fail");
    assert_eq!(
        conflict,
        "expected revision 1, but the current revision is 0"
    );
    assert_eq!(project_state_signature(&project), before);

    let invalid = execute_command(
        &mut project,
        project_id,
        0,
        Command::ResizeRectangularPaper {
            width_mm: 0.0,
            height_mm: 297.0,
        },
    )
    .expect_err("zero width must fail");
    assert_eq!(invalid, "paper width must be greater than zero");
    assert_eq!(project_state_signature(&project), before);

    let overflow = execute_command(
        &mut project,
        project_id,
        0,
        Command::ResizeRectangularPaper {
            width_mm: f64::MAX,
            height_mm: 2.0,
        },
    )
    .expect_err("unrepresentable area must fail");
    assert_eq!(
        overflow,
        "target paper area is too large to represent safely"
    );
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn generated_id_edge_split_updates_snapshot_document_and_history() {
    let sheet = create_rectangular_sheet(100.0, 80.0, false).expect("valid rectangle");
    let (mut pattern, paper) = sheet.into_parts();
    let crease = Edge {
        id: EdgeId::new(),
        start: paper.boundary_vertices[0],
        end: paper.boundary_vertices[2],
        kind: EdgeKind::Valley,
    };
    pattern.edges.push(crease.clone());
    let original_vertex_ids = pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let original_edge_ids = pattern.edges.iter().map(|edge| edge.id).collect::<Vec<_>>();
    let original_edge_index = pattern.edges.len() - 1;
    let mut project = ProjectState::new_with_paper(pattern, paper);
    let project_id = project.project_id;
    let original_document = project.document();

    let response =
        execute_edge_split(&mut project, project_id, 0, crease.id, 0.5).expect("split crease edge");

    assert_eq!(response.revision, 1);
    assert!(response.is_dirty);
    assert!(response.can_undo);
    assert!(!response.can_redo);
    assert_eq!(response.paper, original_document.paper);
    assert_eq!(response.crease_pattern.vertices.len(), 5);
    let generated_vertices = response
        .crease_pattern
        .vertices
        .iter()
        .filter(|vertex| !original_vertex_ids.contains(&vertex.id))
        .collect::<Vec<_>>();
    assert_eq!(generated_vertices.len(), 1);
    let generated_vertex = generated_vertices[0];
    assert_eq!(generated_vertex.position, Point2::new(50.0, 40.0));
    assert_eq!(response.crease_pattern.edges.len(), 6);
    assert_eq!(
        response.crease_pattern.edges[original_edge_index],
        Edge {
            end: generated_vertex.id,
            ..crease.clone()
        }
    );
    let generated_edge = &response.crease_pattern.edges[original_edge_index + 1];
    assert!(!original_edge_ids.contains(&generated_edge.id));
    assert_eq!(generated_edge.start, generated_vertex.id);
    assert_eq!(generated_edge.end, crease.end);
    assert_eq!(generated_edge.kind, EdgeKind::Valley);
    assert!(validation_snapshot(&project).is_valid);
    let split_document = project.document();
    assert_ne!(split_document, original_document);

    project.editor.undo(1).expect("undo edge split");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project.editor.redo(2).expect("redo edge split");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), split_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn edge_split_conflicts_invalid_fractions_and_boundary_targets_preserve_project_state() {
    let sheet = create_rectangular_sheet(100.0, 80.0, false).expect("valid rectangle");
    let (mut pattern, paper) = sheet.into_parts();
    let boundary_edge = pattern.edges[0].id;
    let crease = Edge {
        id: EdgeId::new(),
        start: paper.boundary_vertices[0],
        end: paper.boundary_vertices[2],
        kind: EdgeKind::Mountain,
    };
    pattern.edges.push(crease.clone());
    let mut project = ProjectState::new_with_paper(pattern, paper);
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    let conflict = execute_edge_split(&mut project, project_id, 1, crease.id, 0.5)
        .expect_err("stale split must fail");
    assert_eq!(
        conflict,
        "expected revision 1, but the current revision is 0"
    );
    assert_eq!(project_state_signature(&project), before);

    let invalid = execute_edge_split(&mut project, project_id, 0, crease.id, f64::NAN)
        .expect_err("non-finite split must fail");
    assert_eq!(invalid, "edge split fraction must be finite");
    assert_eq!(project_state_signature(&project), before);

    let boundary = execute_edge_split(&mut project, project_id, 0, boundary_edge, 0.5)
        .expect_err("boundary split must use the sheet command");
    assert!(boundary.contains("must be changed through a sheet-boundary operation"));
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn edge_intersection_connection_returns_vertex_and_exact_undoable_snapshot() {
    let (mut project, first, second) = crossing_project();
    let project_id = project.project_id;
    let original_document = project.document();
    let original_vertex_ids = original_document
        .crease_pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let original_edge_ids = original_document
        .crease_pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<Vec<_>>();

    let response =
        execute_edge_intersection_connection(&mut project, project_id, 0, second.id, first.id)
            .expect("connect crossing edges");

    assert_eq!(response.snapshot.revision, 1);
    assert!(response.snapshot.is_dirty);
    assert!(response.snapshot.can_undo);
    assert!(!response.snapshot.can_redo);
    let created_vertex = response
        .snapshot
        .crease_pattern
        .vertices
        .iter()
        .find(|vertex| vertex.id == response.vertex_id)
        .expect("explicitly returned generated vertex");
    assert_eq!(created_vertex.position, Point2::new(50.0, 50.0));
    assert!(!original_vertex_ids.contains(&response.vertex_id));
    let generated_edges = response
        .snapshot
        .crease_pattern
        .edges
        .iter()
        .filter(|edge| !original_edge_ids.contains(&edge.id))
        .collect::<Vec<_>>();
    assert_eq!(generated_edges.len(), 2);
    assert!(
        generated_edges
            .iter()
            .all(|edge| edge.start == response.vertex_id)
    );
    assert_eq!(
        generated_edges
            .iter()
            .map(|edge| edge.kind)
            .collect::<Vec<_>>(),
        vec![EdgeKind::Mountain, EdgeKind::Valley]
    );
    assert_eq!(
        response.snapshot.crease_pattern,
        project.editor.pattern().clone()
    );
    assert!(validation_snapshot(&project).is_valid);
    let connected_document = project.document();

    project
        .editor
        .undo(1)
        .expect("undo intersection connection");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project
        .editor
        .redo(2)
        .expect("redo intersection connection");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), connected_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn edge_intersection_api_rejections_preserve_entire_project_state() {
    let (mut project, first, second) = crossing_project();
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    let wrong_project = execute_edge_intersection_connection(
        &mut project,
        ProjectId::new(),
        0,
        first.id,
        second.id,
    )
    .expect_err("wrong project must fail");
    assert!(wrong_project.contains("active project changed"));
    assert_eq!(project_state_signature(&project), before);

    let stale =
        execute_edge_intersection_connection(&mut project, project_id, 4, first.id, second.id)
            .expect_err("stale revision must fail");
    assert_eq!(stale, "expected revision 4, but the current revision is 0");
    assert_eq!(project_state_signature(&project), before);

    let same_edge =
        execute_edge_intersection_connection(&mut project, project_id, 0, first.id, first.id)
            .expect_err("same target edge must fail");
    assert_eq!(same_edge, "the two intersection edge IDs must be different");
    assert_eq!(project_state_signature(&project), before);

    let boundary = project.editor.pattern().edges[0].id;
    let boundary_error =
        execute_edge_intersection_connection(&mut project, project_id, 0, boundary, first.id)
            .expect_err("boundary target must fail");
    assert!(boundary_error.contains("must not be a boundary edge"));
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn edge_intersection_api_rejects_t_junction_without_mutation() {
    let (project, first, second) = crossing_project();
    let mut document = project.document();
    document
        .crease_pattern
        .vertices
        .iter_mut()
        .find(|vertex| vertex.id == second.start)
        .expect("second start")
        .position = Point2::new(50.0, 50.0);
    let mut project = ProjectState::new_with_paper(document.crease_pattern, document.paper);
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    let error =
        execute_edge_intersection_connection(&mut project, project_id, 0, first.id, second.id)
            .expect_err("T-junction must fail");

    assert_eq!(
        error,
        "the selected edges must intersect strictly inside both edges"
    );
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn intersection_cluster_api_creates_three_way_junction_with_one_step_history() {
    let (mut project, edges) = create_cluster_project(false);
    let project_id = project.project_id;
    let original_document = project.document();
    let original_vertex_ids = original_document
        .crease_pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let original_edge_ids = original_document
        .crease_pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<Vec<_>>();
    let targets = edges
        .iter()
        .map(|edge| IntersectionClusterTargetRequest {
            edge_id: edge.id,
            relation: IntersectionClusterRelation::Interior,
        })
        .collect();

    let response =
        execute_intersection_cluster_connection(&mut project, project_id, 0, targets, None)
            .expect("connect a newly created three-edge intersection cluster");

    assert_eq!(response.snapshot.revision, 1);
    assert!(response.snapshot.is_dirty);
    assert!(response.snapshot.can_undo);
    assert!(!response.snapshot.can_redo);
    assert_eq!(response.snapshot.paper, original_document.paper);
    assert!(!original_vertex_ids.contains(&response.vertex_id));
    assert_eq!(
        response
            .snapshot
            .crease_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == response.vertex_id)
            .expect("created cluster junction")
            .position,
        Point2::new(50.0, 50.0)
    );
    assert_eq!(
        response.snapshot.crease_pattern.vertices.len(),
        original_document.crease_pattern.vertices.len() + 1
    );
    assert_eq!(
        response.snapshot.crease_pattern.edges.len(),
        original_document.crease_pattern.edges.len() + edges.len()
    );
    for edge in &edges {
        let split_original = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|candidate| candidate.id == edge.id)
            .expect("split original cluster edge");
        assert_eq!(split_original.start, edge.start);
        assert_eq!(split_original.end, response.vertex_id);
        assert_eq!(split_original.kind, edge.kind);
        let generated = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|candidate| {
                !original_edge_ids.contains(&candidate.id)
                    && candidate.start == response.vertex_id
                    && candidate.end == edge.end
            })
            .expect("generated cluster edge");
        assert_eq!(generated.kind, edge.kind);
    }
    assert!(validation_snapshot(&project).is_valid);
    let connected_document = project.document();

    project
        .editor
        .undo(1)
        .expect("undo created intersection cluster");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project
        .editor
        .redo(2)
        .expect("redo created intersection cluster");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), connected_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn intersection_cluster_api_accepts_64_targets_and_returns_the_created_junction() {
    let (mut project, edges) = maximum_cluster_project();
    assert_eq!(edges.len(), MAX_INTERSECTION_CLUSTER_TARGETS);
    let project_id = project.project_id;
    let original_document = project.document();
    let original_vertex_ids = original_document
        .crease_pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let targets = edges
        .iter()
        .map(|edge| IntersectionClusterTargetRequest {
            edge_id: edge.id,
            relation: IntersectionClusterRelation::Interior,
        })
        .collect();

    let response =
        execute_intersection_cluster_connection(&mut project, project_id, 0, targets, None)
            .expect("the inclusive 64-target API limit must connect");

    assert_eq!(response.snapshot.revision, 1);
    assert!(response.snapshot.is_dirty);
    assert!(response.snapshot.can_undo);
    assert!(!response.snapshot.can_redo);
    assert!(!original_vertex_ids.contains(&response.vertex_id));
    assert_eq!(
        response
            .snapshot
            .crease_pattern
            .vertices
            .iter()
            .find(|vertex| vertex.id == response.vertex_id),
        Some(&Vertex {
            id: response.vertex_id,
            position: Point2::new(50.0, 50.0),
        })
    );
    assert_eq!(
        response.snapshot.crease_pattern.vertices.len(),
        original_document.crease_pattern.vertices.len() + 1
    );
    assert_eq!(
        response.snapshot.crease_pattern.edges.len(),
        original_document.crease_pattern.edges.len() + MAX_INTERSECTION_CLUSTER_TARGETS
    );
    for source in &edges {
        let split_original = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|edge| edge.id == source.id)
            .expect("each maximum-cluster source edge remains");
        assert_eq!(split_original.start, source.start);
        assert_eq!(split_original.end, response.vertex_id);
        assert_eq!(split_original.kind, source.kind);
        let generated = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|edge| {
                !edges.iter().any(|source| source.id == edge.id)
                    && edge.start == response.vertex_id
                    && edge.end == source.end
            })
            .expect("each maximum-cluster source gets one generated half");
        assert_eq!(generated.kind, source.kind);
    }
    assert!(validation_snapshot(&project).is_valid);

    let (mut rejected_project, rejected_edges) = maximum_cluster_project();
    let rejected_project_id = rejected_project.project_id;
    let rejected_before = project_state_signature(&rejected_project);
    let error = execute_intersection_cluster_connection(
        &mut rejected_project,
        rejected_project_id,
        0,
        (0..=MAX_INTERSECTION_CLUSTER_TARGETS)
            .map(|index| IntersectionClusterTargetRequest {
                edge_id: rejected_edges[index % rejected_edges.len()].id,
                relation: IntersectionClusterRelation::Interior,
            })
            .collect(),
        None,
    )
    .expect_err("65 targets must be rejected at the API boundary");
    assert_eq!(
        error,
        "an intersection cluster supports at most 64 target edges, found 65"
    );
    assert_eq!(project_state_signature(&rejected_project), rejected_before);
}

#[test]
fn intersection_cluster_api_reuses_junction_with_interior_and_endpoint_targets() {
    let (mut project, [horizontal, vertical, stem], junction) = reuse_cluster_project();
    let project_id = project.project_id;
    let original_document = project.document();
    let original_edge_ids = original_document
        .crease_pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<Vec<_>>();
    let targets = vec![
        IntersectionClusterTargetRequest {
            edge_id: stem.id,
            relation: IntersectionClusterRelation::Endpoint,
        },
        IntersectionClusterTargetRequest {
            edge_id: vertical.id,
            relation: IntersectionClusterRelation::Interior,
        },
        IntersectionClusterTargetRequest {
            edge_id: horizontal.id,
            relation: IntersectionClusterRelation::Interior,
        },
    ];

    let response = execute_intersection_cluster_connection(
        &mut project,
        project_id,
        0,
        targets,
        Some(junction),
    )
    .expect("connect a mixed interior/endpoint cluster at an existing vertex");

    assert_eq!(response.vertex_id, junction);
    assert_eq!(response.snapshot.revision, 1);
    assert!(response.snapshot.is_dirty);
    assert!(response.snapshot.can_undo);
    assert!(!response.snapshot.can_redo);
    assert_eq!(
        response.snapshot.crease_pattern.vertices,
        original_document.crease_pattern.vertices
    );
    assert_eq!(
        response.snapshot.crease_pattern.edges.len(),
        original_document.crease_pattern.edges.len() + 2
    );
    assert!(
        response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .any(|edge| edge == &stem)
    );
    for edge in [&horizontal, &vertical] {
        let split_original = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|candidate| candidate.id == edge.id)
            .expect("split original cluster edge");
        assert_eq!(split_original.start, edge.start);
        assert_eq!(split_original.end, junction);
        assert_eq!(split_original.kind, edge.kind);
        let generated = response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .find(|candidate| {
                !original_edge_ids.contains(&candidate.id)
                    && candidate.start == junction
                    && candidate.end == edge.end
            })
            .expect("generated cluster edge");
        assert_eq!(generated.kind, edge.kind);
    }
    assert!(validation_snapshot(&project).is_valid);
    let connected_document = project.document();

    project
        .editor
        .undo(1)
        .expect("undo reused intersection cluster");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project
        .editor
        .redo(2)
        .expect("redo reused intersection cluster");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), connected_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn intersection_cluster_api_rejections_are_atomic_and_boundary_remains_unsupported() {
    let interior_target = |edge: &Edge| IntersectionClusterTargetRequest {
        edge_id: edge.id,
        relation: IntersectionClusterRelation::Interior,
    };

    let (mut bounded_project, bounded_edges) = create_cluster_project(false);
    let bounded_project_id = bounded_project.project_id;
    let bounded_before = project_state_signature(&bounded_project);
    let too_few_error = execute_intersection_cluster_connection(
        &mut bounded_project,
        bounded_project_id,
        0,
        bounded_edges[..2].iter().map(interior_target).collect(),
        None,
    )
    .expect_err("fewer than three request targets must fail before ID allocation");
    assert_eq!(
        too_few_error,
        "an intersection cluster requires at least three target edges, found 2"
    );
    let too_many_error = execute_intersection_cluster_connection(
        &mut bounded_project,
        bounded_project_id,
        0,
        (0..65)
            .map(|_| interior_target(&bounded_edges[0]))
            .collect(),
        None,
    )
    .expect_err("more than 64 request targets must fail before ID allocation");
    assert_eq!(
        too_many_error,
        "an intersection cluster supports at most 64 target edges, found 65"
    );
    assert_eq!(project_state_signature(&bounded_project), bounded_before);

    let (mut stale_project, stale_edges) = create_cluster_project(false);
    let stale_project_id = stale_project.project_id;
    let stale_before = project_state_signature(&stale_project);
    let stale_error = execute_intersection_cluster_connection(
        &mut stale_project,
        stale_project_id,
        1,
        stale_edges.iter().map(interior_target).collect(),
        None,
    )
    .expect_err("stale cluster command must fail");
    assert_eq!(
        stale_error,
        "expected revision 1, but the current revision is 0"
    );
    assert_eq!(project_state_signature(&stale_project), stale_before);

    let (mut incomplete_project, incomplete_edges) = create_cluster_project(true);
    let incomplete_project_id = incomplete_project.project_id;
    let incomplete_before = project_state_signature(&incomplete_project);
    let incomplete_error = execute_intersection_cluster_connection(
        &mut incomplete_project,
        incomplete_project_id,
        0,
        incomplete_edges[..3].iter().map(interior_target).collect(),
        None,
    )
    .expect_err("an omitted intersecting edge must reject the whole cluster");
    assert!(incomplete_error.contains("also passes through the intersection cluster"));
    assert!(incomplete_error.contains(&format!("{:?}", incomplete_edges[3].id)));
    assert_eq!(
        project_state_signature(&incomplete_project),
        incomplete_before
    );

    let (mut boundary_project, boundary_edges) = create_cluster_project(false);
    let boundary_project_id = boundary_project.project_id;
    let boundary_before = project_state_signature(&boundary_project);
    let boundary = boundary_project.editor.pattern().edges[0].clone();
    let boundary_error = execute_intersection_cluster_connection(
        &mut boundary_project,
        boundary_project_id,
        0,
        vec![
            interior_target(&boundary),
            interior_target(&boundary_edges[1]),
            interior_target(&boundary_edges[2]),
        ],
        None,
    )
    .expect_err("boundary clusters remain unsupported in the first core increment");
    assert!(boundary_error.contains("does not yet support boundary edge"));
    assert_eq!(project_state_signature(&boundary_project), boundary_before);
}

#[test]
fn t_junction_connection_returns_reused_vertex_and_undoable_dirty_snapshot() {
    let (mut project, interior, stem, junction) = t_junction_project();
    let project_id = project.project_id;
    let original_document = project.document();
    let original_vertex_count = original_document.crease_pattern.vertices.len();
    let original_edge_ids = original_document
        .crease_pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<Vec<_>>();

    let response = execute_t_junction_connection(&mut project, project_id, 0, stem.id, interior.id)
        .expect("connect T-junction with reverse arguments");

    assert_eq!(response.vertex_id, junction);
    assert_eq!(response.snapshot.revision, 1);
    assert!(response.snapshot.is_dirty);
    assert!(response.snapshot.can_undo);
    assert!(!response.snapshot.can_redo);
    assert_eq!(
        response.snapshot.crease_pattern.vertices.len(),
        original_vertex_count
    );
    assert_eq!(
        response.snapshot.crease_pattern.vertices,
        original_document.crease_pattern.vertices
    );
    let split_original = response
        .snapshot
        .crease_pattern
        .edges
        .iter()
        .find(|edge| edge.id == interior.id)
        .expect("split original edge");
    assert_eq!(split_original.start, interior.start);
    assert_eq!(split_original.end, junction);
    assert_eq!(split_original.kind, EdgeKind::Mountain);
    let generated = response
        .snapshot
        .crease_pattern
        .edges
        .iter()
        .find(|edge| !original_edge_ids.contains(&edge.id))
        .expect("generated T-junction edge");
    assert_eq!(generated.start, junction);
    assert_eq!(generated.end, interior.end);
    assert_eq!(generated.kind, EdgeKind::Mountain);
    assert!(
        response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .any(|edge| edge == &stem)
    );
    assert!(validation_snapshot(&project).is_valid);
    let connected_document = project.document();

    project.editor.undo(1).expect("undo T-junction connection");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project.editor.redo(2).expect("redo T-junction connection");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), connected_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn boundary_t_junction_api_splits_sheet_outline_with_reused_vertex_and_exact_history() {
    let (mut project, boundary, stem, junction) = boundary_t_junction_project();
    let project_id = project.project_id;
    let original_document = project.document();
    let original_vertex_count = original_document.crease_pattern.vertices.len();
    let original_edge_ids = original_document
        .crease_pattern
        .edges
        .iter()
        .map(|edge| edge.id)
        .collect::<Vec<_>>();
    let original_boundary_vertices = original_document.paper.boundary_vertices.clone();

    let response = execute_t_junction_connection(&mut project, project_id, 0, stem.id, boundary.id)
        .expect("connect a crease endpoint to the strict interior of the sheet boundary");

    assert_eq!(response.vertex_id, junction);
    assert_eq!(response.snapshot.revision, 1);
    assert!(response.snapshot.is_dirty);
    assert!(response.snapshot.can_undo);
    assert!(!response.snapshot.can_redo);
    assert_eq!(
        response.snapshot.crease_pattern.vertices.len(),
        original_vertex_count
    );
    assert_eq!(
        response.snapshot.crease_pattern.vertices,
        original_document.crease_pattern.vertices
    );
    assert_eq!(
        response.snapshot.paper.boundary_vertices,
        vec![
            original_boundary_vertices[0],
            junction,
            original_boundary_vertices[1],
            original_boundary_vertices[2],
            original_boundary_vertices[3],
        ]
    );

    let split_original = response
        .snapshot
        .crease_pattern
        .edges
        .iter()
        .find(|edge| edge.id == boundary.id)
        .expect("original boundary segment");
    assert_eq!(split_original.start, boundary.start);
    assert_eq!(split_original.end, junction);
    assert_eq!(split_original.kind, EdgeKind::Boundary);
    let generated = response
        .snapshot
        .crease_pattern
        .edges
        .iter()
        .find(|edge| !original_edge_ids.contains(&edge.id))
        .expect("generated boundary segment");
    assert_eq!(generated.start, junction);
    assert_eq!(generated.end, boundary.end);
    assert_eq!(generated.kind, EdgeKind::Boundary);
    assert!(
        response
            .snapshot
            .crease_pattern
            .edges
            .iter()
            .any(|edge| edge == &stem)
    );
    assert!(validation_snapshot(&project).is_valid);
    let connected_document = project.document();

    project
        .editor
        .undo(1)
        .expect("undo boundary T-junction connection");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project
        .editor
        .redo(2)
        .expect("redo boundary T-junction connection");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), connected_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn t_junction_api_conflicts_and_wrong_geometry_preserve_project_state() {
    let (mut project, interior, stem, _) = t_junction_project();
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    let wrong_project =
        execute_t_junction_connection(&mut project, ProjectId::new(), 0, interior.id, stem.id)
            .expect_err("wrong project must fail");
    assert!(wrong_project.contains("active project changed"));
    assert_eq!(project_state_signature(&project), before);

    let stale = execute_t_junction_connection(&mut project, project_id, 3, interior.id, stem.id)
        .expect_err("stale revision must fail");
    assert_eq!(stale, "expected revision 3, but the current revision is 0");
    assert_eq!(project_state_signature(&project), before);

    let boundary = project.editor.pattern().edges[0].id;
    let boundary_error =
        execute_t_junction_connection(&mut project, project_id, 0, boundary, interior.id)
            .expect_err("non-intersecting boundary target must fail");
    assert_eq!(
        boundary_error,
        "the selected edges do not form exactly one strict T-junction"
    );
    assert_eq!(project_state_signature(&project), before);

    let (mut crossing, first, second) = crossing_project();
    let crossing_project_id = crossing.project_id;
    let crossing_before = project_state_signature(&crossing);
    let proper_x =
        execute_t_junction_connection(&mut crossing, crossing_project_id, 0, first.id, second.id)
            .expect_err("proper X must not be accepted as T-junction");
    assert_eq!(
        proper_x,
        "the selected edges do not form exactly one strict T-junction"
    );
    assert_eq!(project_state_signature(&crossing), crossing_before);
}

#[test]
fn generated_id_boundary_split_handles_reverse_closing_edge_and_document_history() {
    let sheet = create_rectangular_sheet(100.0, 80.0, false).expect("valid rectangle");
    let (mut pattern, paper) = sheet.into_parts();
    let forward_closing_edge = pattern.edges[3].clone();
    pattern.edges[3] = Edge {
        start: forward_closing_edge.end,
        end: forward_closing_edge.start,
        ..forward_closing_edge
    };
    let target_edge = pattern.edges[3].clone();
    let original_vertex_ids = pattern
        .vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let original_edge_ids = pattern.edges.iter().map(|edge| edge.id).collect::<Vec<_>>();
    let mut project = ProjectState::new_with_paper(pattern, paper);
    let project_id = project.project_id;
    let original_document = project.document();

    let response = execute_boundary_split(&mut project, project_id, 0, target_edge.id, 0.25)
        .expect("split reverse closing edge");

    assert_eq!(response.revision, 1);
    assert!(response.is_dirty);
    assert!(response.can_undo);
    assert!(!response.can_redo);
    assert_eq!(response.paper.boundary_vertices.len(), 5);
    let new_vertex = response.paper.boundary_vertices[4];
    assert!(!original_vertex_ids.contains(&new_vertex));
    assert_eq!(response.crease_pattern.vertices.len(), 5);
    assert_eq!(
        response.crease_pattern.vertices[4],
        Vertex {
            id: new_vertex,
            position: Point2::new(0.0, 20.0),
        }
    );
    assert_eq!(response.crease_pattern.edges.len(), 5);
    assert_eq!(response.crease_pattern.edges[3].id, target_edge.id);
    assert_eq!(response.crease_pattern.edges[3].start, target_edge.start);
    assert_eq!(response.crease_pattern.edges[3].end, new_vertex);
    let generated_edge = &response.crease_pattern.edges[4];
    assert!(!original_edge_ids.contains(&generated_edge.id));
    assert_eq!(generated_edge.start, new_vertex);
    assert_eq!(generated_edge.end, target_edge.end);
    assert_eq!(generated_edge.kind, EdgeKind::Boundary);
    assert!(validation_snapshot(&project).is_valid);
    let split_document = project.document();
    assert_ne!(split_document, original_document);

    project.editor.undo(1).expect("undo boundary split");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project.editor.redo(2).expect("redo boundary split");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), split_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn boundary_split_conflict_and_invalid_fraction_preserve_project_state() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let edge = project.editor.pattern().edges[0].id;
    let before = project_state_signature(&project);

    let conflict = execute_boundary_split(&mut project, project_id, 1, edge, 0.5)
        .expect_err("stale split must fail");
    assert_eq!(
        conflict,
        "expected revision 1, but the current revision is 0"
    );
    assert_eq!(project_state_signature(&project), before);

    let invalid = execute_boundary_split(&mut project, project_id, 0, edge, f64::NAN)
        .expect_err("non-finite split must fail");
    assert_eq!(invalid, "boundary split fraction must be finite");
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn boundary_vertex_removal_updates_document_dirty_state_and_history() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let original_document = project.document();
    let target = project.editor.paper().boundary_vertices[1];
    let previous = project.editor.paper().boundary_vertices[0];
    let next = project.editor.paper().boundary_vertices[2];
    let remaining = project.editor.paper().boundary_vertices[3];
    let kept_edge = project.editor.pattern().edges[0].clone();
    let removed_edge = project.editor.pattern().edges[1].clone();

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::RemoveBoundaryVertex { vertex: target },
    )
    .expect("remove boundary vertex");

    assert_eq!(response.revision, 1);
    assert!(response.is_dirty);
    assert!(response.can_undo);
    assert!(!response.can_redo);
    assert_eq!(
        response.paper.boundary_vertices,
        vec![previous, next, remaining]
    );
    assert!(
        !response
            .crease_pattern
            .vertices
            .iter()
            .any(|vertex| vertex.id == target)
    );
    assert_eq!(response.crease_pattern.edges[0].id, kept_edge.id);
    assert_eq!(response.crease_pattern.edges[0].start, previous);
    assert_eq!(response.crease_pattern.edges[0].end, next);
    assert!(
        !response
            .crease_pattern
            .edges
            .iter()
            .any(|edge| edge.id == removed_edge.id)
    );
    assert!(validation_snapshot(&project).is_valid);
    let removed_document = project.document();
    assert_ne!(removed_document, original_document);

    project.editor.undo(1).expect("undo boundary removal");
    assert_eq!(project.editor.revision(), 2);
    assert_eq!(project.document(), original_document);
    assert!(!project.is_dirty());

    project.editor.redo(2).expect("redo boundary removal");
    assert_eq!(project.editor.revision(), 3);
    assert_eq!(project.document(), removed_document);
    assert!(project.is_dirty());
    assert!(validation_snapshot(&project).is_valid);
}

#[test]
fn boundary_vertex_removal_conflict_preserves_project_state() {
    let mut project = initial_project_state();
    let project_id = project.project_id;
    let target = project.editor.paper().boundary_vertices[1];
    let before = project_state_signature(&project);

    let error = execute_command(
        &mut project,
        project_id,
        1,
        Command::RemoveBoundaryVertex { vertex: target },
    )
    .expect_err("stale boundary removal must fail");

    assert_eq!(error, "expected revision 1, but the current revision is 0");
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn new_project_replaces_only_the_expected_unchanged_project() {
    let mut project = initial_project_state();
    let old_instance_id = project.instance_id;
    let old_project_id = project.project_id;

    let response = replace_with_new_project(
        &mut project,
        old_instance_id,
        old_project_id,
        0,
        new_project_parameters(),
    )
    .expect("replace current project");

    assert_ne!(response.project_id, old_project_id);
    assert_eq!(response.project_id, project.project_id);
    assert_eq!(response.name, "Test sheet");
    assert!(response.current_path.is_none());
    assert_eq!(response.revision, 0);
    assert!(response.saved_revision.is_none());
    assert!(response.is_dirty);
    assert!(!response.can_undo);
    assert!(!response.can_redo);
    assert!(project.saved_document.is_none());
}

#[test]
fn new_project_errors_leave_existing_state_untouched() {
    let mut project = initial_project_state();
    let instance_id = project.instance_id;
    let project_id = project.project_id;
    let before = project_state_signature(&project);

    assert!(
        replace_with_new_project(
            &mut project,
            instance_id,
            ProjectId::new(),
            0,
            new_project_parameters(),
        )
        .is_err()
    );
    assert_eq!(project_state_signature(&project), before);

    assert!(
        replace_with_new_project(
            &mut project,
            instance_id,
            project_id,
            1,
            new_project_parameters(),
        )
        .is_err()
    );
    assert_eq!(project_state_signature(&project), before);

    let mut invalid_name = new_project_parameters();
    invalid_name.name = " \0 ".to_owned();
    assert!(
        replace_with_new_project(&mut project, instance_id, project_id, 0, invalid_name).is_err()
    );
    assert_eq!(project_state_signature(&project), before);

    let mut invalid_dimensions = new_project_parameters();
    invalid_dimensions.width_mm = 0.0;
    assert!(
        replace_with_new_project(&mut project, instance_id, project_id, 0, invalid_dimensions,)
            .is_err()
    );
    assert_eq!(project_state_signature(&project), before);

    let mut invalid_thickness = new_project_parameters();
    invalid_thickness.thickness_mm = f64::NAN;
    assert!(
        replace_with_new_project(&mut project, instance_id, project_id, 0, invalid_thickness,)
            .is_err()
    );
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn delayed_new_project_rejects_same_document_revision_after_reopen_aba() {
    let mut project = initial_project_state();
    let stale_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let document = project.document();
    project = ProjectState::from_valid_document(document, PathBuf::from("same-project.ori2"));
    assert_eq!(project.project_id, expected_project_id);
    assert_eq!(project.editor.revision(), expected_revision);
    assert_ne!(project.instance_id, stale_instance_id);
    let before = project_state_signature(&project);

    let error = replace_with_new_project(
        &mut project,
        stale_instance_id,
        expected_project_id,
        expected_revision,
        new_project_parameters(),
    )
    .expect_err("reopened ABA instance must reject delayed new-project work");

    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn execute_command_rejects_same_document_revision_after_reopen_aba() {
    let project = initial_project_state();
    let stale_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let mut reopened =
        ProjectState::from_valid_document(project.document(), PathBuf::from("same-project.ori2"));
    assert_eq!(reopened.project_id, expected_project_id);
    assert_eq!(reopened.editor.revision(), expected_revision);
    assert_ne!(reopened.instance_id, stale_instance_id);
    let before = project_state_signature(&reopened);

    let error = super::execute_command(
        &mut reopened,
        stale_instance_id,
        expected_project_id,
        expected_revision,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(25.0, 25.0),
        },
    )
    .expect_err("reopened ABA instance must reject a delayed edit command");

    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&reopened), before);
}

#[test]
fn execute_undo_rejects_same_project_and_revision_from_a_foreign_instance() {
    let mut stale_project = initial_project_state();
    let expected_project_id = stale_project.project_id;
    execute_command(
        &mut stale_project,
        expected_project_id,
        0,
        Command::SetCuttingAllowed { allowed: true },
    )
    .expect("advance the stale project to revision one");
    let stale_instance_id = stale_project.instance_id;
    let expected_revision = stale_project.editor.revision();

    let mut reopened = ProjectState::from_valid_document(
        stale_project.document(),
        PathBuf::from("same-project.ori2"),
    );
    execute_command(
        &mut reopened,
        expected_project_id,
        0,
        Command::SetCuttingAllowed { allowed: false },
    )
    .expect("create undo history at the same revision");
    assert_eq!(reopened.editor.revision(), expected_revision);
    assert!(reopened.editor.can_undo());
    assert_ne!(reopened.instance_id, stale_instance_id);
    let before = project_state_signature(&reopened);

    let error = super::execute_undo(
        &mut reopened,
        stale_instance_id,
        expected_project_id,
        expected_revision,
    )
    .expect_err("foreign project instance must not consume undo history");

    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&reopened), before);
}

#[test]
fn execute_redo_rejects_same_project_and_revision_from_a_foreign_instance() {
    let mut stale_project = initial_project_state();
    let expected_project_id = stale_project.project_id;
    execute_command(
        &mut stale_project,
        expected_project_id,
        0,
        Command::SetCuttingAllowed { allowed: true },
    )
    .expect("advance the stale project to revision one");
    execute_command(
        &mut stale_project,
        expected_project_id,
        1,
        Command::SetCuttingAllowed { allowed: false },
    )
    .expect("advance the stale project to revision two");
    let stale_instance_id = stale_project.instance_id;
    let expected_revision = stale_project.editor.revision();

    let mut reopened = ProjectState::from_valid_document(
        stale_project.document(),
        PathBuf::from("same-project.ori2"),
    );
    execute_command(
        &mut reopened,
        expected_project_id,
        0,
        Command::SetCuttingAllowed { allowed: true },
    )
    .expect("create current-instance undo history");
    execute_undo(&mut reopened, expected_project_id, 1)
        .expect("create redo history at revision two");
    assert_eq!(reopened.editor.revision(), expected_revision);
    assert!(reopened.editor.can_redo());
    assert_ne!(reopened.instance_id, stale_instance_id);
    let before = project_state_signature(&reopened);

    let error = super::execute_redo(
        &mut reopened,
        stale_instance_id,
        expected_project_id,
        expected_revision,
    )
    .expect_err("foreign project instance must not consume redo history");

    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&reopened), before);
}

#[test]
fn move_vertex_returns_the_updated_revision_and_snapshot() {
    let id = VertexId::new();
    let mut project = ProjectState::new(CreasePattern {
        vertices: vec![Vertex {
            id,
            position: Point2::new(1.0, 2.0),
        }],
        edges: Vec::new(),
    });
    let project_id = project.project_id;
    assert!(!project.is_dirty());

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::MoveVertex {
            id,
            position: Point2::new(3.0, 5.0),
        },
    )
    .expect("move vertex");

    assert_eq!(response.revision, 1);
    assert_eq!(
        response.crease_pattern.vertices[0].position,
        Point2::new(3.0, 5.0)
    );
    assert!(response.can_undo);
    assert!(response.is_dirty);
}

#[test]
fn face_vertex_batch_is_one_persisted_undo_redo_entry() {
    let first = VertexId::new();
    let second = VertexId::new();
    let edge = EdgeId::new();
    let mut project = ProjectState::new_unsaved(
        "face batch".to_owned(),
        CreasePattern {
            vertices: vec![
                ori_domain::Vertex {
                    id: first,
                    position: Point2::new(1.0, 2.0),
                },
                ori_domain::Vertex {
                    id: second,
                    position: Point2::new(3.0, 4.0),
                },
            ],
            edges: vec![ori_domain::Edge {
                id: edge,
                start: first,
                end: second,
                kind: EdgeKind::Mountain,
            }],
        },
        Paper::default(),
    );
    let project_id = project.project_id;
    execute_command(
        &mut project,
        project_id,
        0,
        Command::MoveVertices {
            updates: vec![
                VertexPositionUpdate {
                    vertex: first,
                    position: Point2::new(11.0, 12.0),
                },
                VertexPositionUpdate {
                    vertex: second,
                    position: Point2::new(13.0, 14.0),
                },
            ],
        },
    )
    .expect("move face vertices");
    let archive = project
        .project_archive()
        .expect("persist face move history");
    let mut reopened =
        ProjectState::from_project_archive(archive, PathBuf::from("face-batch.ori2"))
            .expect("restore face move history");
    assert_eq!(
        reopened.editor.pattern().vertices[0].position,
        Point2::new(11.0, 12.0)
    );
    assert_eq!(
        reopened.editor.pattern().vertices[1].position,
        Point2::new(13.0, 14.0)
    );
    let reopened_project_id = reopened.project_id;
    let undo_revision = reopened.editor.revision();
    execute_undo(&mut reopened, reopened_project_id, undo_revision)
        .expect("undo the face move as one entry");
    assert_eq!(
        reopened.editor.pattern().vertices[0].position,
        Point2::new(1.0, 2.0)
    );
    assert_eq!(
        reopened.editor.pattern().vertices[1].position,
        Point2::new(3.0, 4.0)
    );
    let redo_revision = reopened.editor.revision();
    execute_redo(&mut reopened, reopened_project_id, redo_revision)
        .expect("redo the face move as one entry");
    assert_eq!(
        reopened.editor.pattern().vertices[0].position,
        Point2::new(11.0, 12.0)
    );
    assert_eq!(
        reopened.editor.pattern().vertices[1].position,
        Point2::new(13.0, 14.0)
    );
}

#[test]
fn initial_project_is_a_clean_square_sheet() {
    let project = initial_project_state();
    let snapshot = snapshot(&project);

    assert!(!snapshot.is_dirty);
    assert_eq!(snapshot.revision, 0);
    assert_eq!(project.editor.paper().boundary_vertices.len(), 4);
    assert_eq!(snapshot.crease_pattern.vertices.len(), 4);
    assert_eq!(snapshot.crease_pattern.edges.len(), 4);
    assert!(
        snapshot
            .crease_pattern
            .edges
            .iter()
            .all(|edge| edge.kind == EdgeKind::Boundary)
    );
}

#[test]
fn remove_edge_then_vertex_returns_each_current_snapshot() {
    let start = VertexId::new();
    let end = VertexId::new();
    let edge = EdgeId::new();
    let mut project = ProjectState::new(CreasePattern {
        vertices: vec![
            Vertex {
                id: start,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: end,
                position: Point2::new(1.0, 0.0),
            },
        ],
        edges: vec![Edge {
            id: edge,
            start,
            end,
            kind: EdgeKind::Mountain,
        }],
    });
    let project_id = project.project_id;

    let response = execute_command(
        &mut project,
        project_id,
        0,
        Command::RemoveEdge { id: edge },
    )
    .expect("remove edge");
    assert_eq!(response.revision, 1);
    assert!(response.crease_pattern.edges.is_empty());

    let response = execute_command(
        &mut project,
        project_id,
        1,
        Command::RemoveVertex { id: start },
    )
    .expect("remove vertex");
    assert_eq!(response.revision, 2);
    assert_eq!(response.crease_pattern.vertices.len(), 1);
    assert_eq!(response.crease_pattern.vertices[0].id, end);
}

#[test]
fn edit_commands_preserve_revision_conflict_errors() {
    let id = VertexId::new();
    let mut project = ProjectState::new(CreasePattern {
        vertices: vec![Vertex {
            id,
            position: Point2::new(0.0, 0.0),
        }],
        edges: Vec::new(),
    });
    let project_id = project.project_id;

    let error = execute_command(&mut project, project_id, 4, Command::RemoveVertex { id })
        .expect_err("stale command must fail");

    assert_eq!(error, "expected revision 4, but the current revision is 0");
    assert_eq!(project.editor.pattern().vertices.len(), 1);
}

#[test]
fn validation_snapshot_identifies_both_crossing_edges() {
    let vertices = [
        Vertex {
            id: VertexId::new(),
            position: Point2::new(0.0, 0.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(2.0, 2.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(0.0, 2.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(2.0, 0.0),
        },
    ];
    let first_edge = EdgeId::new();
    let second_edge = EdgeId::new();
    let project = ProjectState::new(CreasePattern {
        vertices: vertices.to_vec(),
        edges: vec![
            Edge {
                id: first_edge,
                start: vertices[0].id,
                end: vertices[1].id,
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: second_edge,
                start: vertices[2].id,
                end: vertices[3].id,
                kind: EdgeKind::Valley,
            },
        ],
    });

    let response = validation_snapshot(&project);

    assert!(!response.is_valid);
    assert_eq!(response.project_id, project.project_id);
    assert_eq!(response.revision, 0);
    assert_eq!(response.issues.len(), 2);
    let crossing = response
        .issues
        .iter()
        .find(|issue| issue.code == "unsplit_intersection")
        .expect("crease-pattern issue");
    assert_eq!(crossing.edges, vec![first_edge, second_edge]);
    assert!(
        response
            .issues
            .iter()
            .any(|issue| issue.code == "too_few_boundary_vertices")
    );
}

#[test]
fn valid_initial_sheet_has_no_combined_validation_issues() {
    let project = initial_project_state();

    let response = validation_snapshot(&project);

    assert!(response.is_valid);
    assert!(response.issues.is_empty());
}

#[test]
fn initial_sheet_reports_boundary_vertices_as_locally_not_applicable() {
    let project = initial_project_state();

    let response = validation_snapshot(&project);
    let encoded = serde_json::to_value(&response).expect("serialize validation snapshot");
    let local = &encoded["local_flat_foldability"];

    assert_eq!(local["model"], "interior_single_vertex_zero_thickness_v1");
    assert_eq!(local["status"], "not_applicable");
    assert_eq!(local["total_vertices"], 4);
    assert_eq!(local["applicable_vertices"], 0);
    assert_eq!(local["not_applicable_vertices"], 4);
    for vertex in local["vertices"].as_array().expect("vertex reports") {
        assert_eq!(vertex["verdict"], "not_applicable");
        assert_eq!(vertex["reason"], "paper_boundary");
        assert_eq!(vertex["kawasaki"], "not_applicable");
        assert_eq!(vertex["maekawa"], "not_applicable");
    }
}

#[test]
fn cardinal_mmmv_vertex_reports_both_local_conditions_satisfied() {
    let (project, center) = four_ray_square_project_state(
        [3, 5, 7, 1],
        [
            EdgeKind::Mountain,
            EdgeKind::Mountain,
            EdgeKind::Mountain,
            EdgeKind::Valley,
        ],
    );

    let response = validation_snapshot(&project);
    let encoded = serde_json::to_value(&response).expect("serialize validation snapshot");
    let center = serde_json::to_value(center).expect("serialize center vertex ID");
    let local = encoded["local_flat_foldability"]
        .as_object()
        .expect("local report object");
    let center_report = local["vertices"]
        .as_array()
        .expect("vertex reports")
        .iter()
        .find(|report| report["vertex"] == center)
        .expect("center report");

    assert_eq!(local["status"], "necessary_conditions_satisfied");
    assert_eq!(local["applicable_vertices"], 1);
    assert_eq!(local["satisfied_vertices"], 1);
    assert_eq!(center_report["fold_degree"], 4);
    assert_eq!(center_report["mountain_count"], 3);
    assert_eq!(center_report["valley_count"], 1);
    assert_eq!(center_report["verdict"], "satisfied");
    assert_eq!(center_report["reason"], serde_json::Value::Null);
    assert_eq!(center_report["kawasaki"], "satisfied");
    assert_eq!(center_report["maekawa"], "satisfied");
}

#[test]
fn local_report_keeps_kawasaki_and_maekawa_violations_independent() {
    let (kawasaki_project, kawasaki_center) = four_ray_square_project_state(
        [3, 5, 7, 0],
        [
            EdgeKind::Mountain,
            EdgeKind::Mountain,
            EdgeKind::Mountain,
            EdgeKind::Valley,
        ],
    );
    let (maekawa_project, maekawa_center) = four_ray_square_project_state(
        [3, 5, 7, 1],
        [
            EdgeKind::Mountain,
            EdgeKind::Mountain,
            EdgeKind::Valley,
            EdgeKind::Valley,
        ],
    );

    let kawasaki = validation_snapshot(&kawasaki_project);
    let kawasaki_json = serde_json::to_value(&kawasaki).expect("serialize Kawasaki counterexample");
    let kawasaki_center =
        serde_json::to_value(kawasaki_center).expect("serialize Kawasaki center vertex ID");
    let kawasaki_center_report = kawasaki_json["local_flat_foldability"]["vertices"]
        .as_array()
        .expect("Kawasaki vertex reports")
        .iter()
        .find(|report| report["vertex"] == kawasaki_center)
        .expect("Kawasaki center report");
    assert_eq!(kawasaki_center_report["kawasaki"], "violated");
    assert_eq!(kawasaki_center_report["maekawa"], "satisfied");
    assert_eq!(kawasaki_center_report["verdict"], "violated");

    let maekawa = validation_snapshot(&maekawa_project);
    let maekawa_json = serde_json::to_value(&maekawa).expect("serialize Maekawa counterexample");
    let maekawa_center =
        serde_json::to_value(maekawa_center).expect("serialize Maekawa center vertex ID");
    let maekawa_center_report = maekawa_json["local_flat_foldability"]["vertices"]
        .as_array()
        .expect("Maekawa vertex reports")
        .iter()
        .find(|report| report["vertex"] == maekawa_center)
        .expect("Maekawa center report");
    assert_eq!(maekawa_center_report["kawasaki"], "satisfied");
    assert_eq!(maekawa_center_report["maekawa"], "violated");
    assert_eq!(maekawa_center_report["verdict"], "violated");
}

#[test]
fn local_flat_foldability_json_contract_is_exact_and_does_not_change_geometry_validity() {
    let (project, center) = four_ray_square_project_state(
        [3, 5, 7, 1],
        [
            EdgeKind::Mountain,
            EdgeKind::Mountain,
            EdgeKind::Valley,
            EdgeKind::Valley,
        ],
    );

    let response = validation_snapshot(&project);
    assert!(response.is_valid);
    assert!(response.issues.is_empty());
    let encoded = serde_json::to_value(&response).expect("serialize validation snapshot");
    let center = serde_json::to_value(center).expect("serialize center vertex ID");
    let root_keys = encoded
        .as_object()
        .expect("validation object")
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let local = encoded["local_flat_foldability"]
        .as_object()
        .expect("local report object");
    let local_keys = local.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let center_report = local["vertices"]
        .as_array()
        .expect("vertex reports")
        .iter()
        .find(|report| report["vertex"] == center)
        .expect("center report")
        .as_object()
        .expect("center report object");
    let center_keys = center_report
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        root_keys,
        [
            "project_id",
            "revision",
            "is_valid",
            "issues",
            "local_flat_foldability"
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        local_keys,
        [
            "model",
            "max_exact_fold_degree",
            "status",
            "total_vertices",
            "applicable_vertices",
            "satisfied_vertices",
            "violated_vertices",
            "not_applicable_vertices",
            "indeterminate_vertices",
            "vertices",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        center_keys,
        [
            "vertex",
            "fold_degree",
            "mountain_count",
            "valley_count",
            "verdict",
            "reason",
            "kawasaki",
            "maekawa",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(local["status"], "violated");
    assert_eq!(center_report["kawasaki"], "satisfied");
    assert_eq!(center_report["maekawa"], "violated");
}

#[test]
fn paper_thickness_issues_are_included_without_highlight_targets() {
    let sheet = create_rectangular_sheet(20.0, 20.0, false).expect("valid square");
    let (pattern, mut paper) = sheet.into_parts();
    paper.thickness_mm = -0.01;
    let project = ProjectState::new_with_paper(pattern.clone(), paper);

    let response = validation_snapshot(&project);

    assert!(!response.is_valid);
    assert_eq!(response.issues.len(), 1);
    assert_eq!(response.issues[0].code, "negative_thickness");
    assert!(response.issues[0].vertices.is_empty());
    assert!(response.issues[0].edges.is_empty());

    let mut zero_paper = project.editor.paper().clone();
    zero_paper.thickness_mm = 0.0;
    let zero_project = ProjectState::new_with_paper(pattern, zero_paper);
    let zero_thickness = validation_snapshot(&zero_project);
    assert!(zero_thickness.is_valid);
    assert!(zero_thickness.issues.is_empty());
}

#[test]
fn paper_intersection_maps_boundary_references_to_domain_edges() {
    let vertices = [
        Vertex {
            id: VertexId::new(),
            position: Point2::new(0.0, 0.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(2.0, 2.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(0.0, 2.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(2.0, 0.0),
        },
    ];
    let boundary_edges = [EdgeId::new(), EdgeId::new(), EdgeId::new(), EdgeId::new()];
    let pattern = CreasePattern {
        vertices: vertices.to_vec(),
        edges: vec![
            Edge {
                id: boundary_edges[0],
                start: vertices[0].id,
                end: vertices[1].id,
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: boundary_edges[1],
                start: vertices[1].id,
                end: vertices[2].id,
                kind: EdgeKind::Boundary,
            },
            // Domain edges are undirected for boundary highlighting, so
            // mapping also accepts the reverse of the paper's order.
            Edge {
                id: boundary_edges[2],
                start: vertices[3].id,
                end: vertices[2].id,
                kind: EdgeKind::Boundary,
            },
            Edge {
                id: boundary_edges[3],
                start: vertices[3].id,
                end: vertices[0].id,
                kind: EdgeKind::Boundary,
            },
        ],
    };
    let paper = Paper {
        boundary_vertices: vertices.iter().map(|vertex| vertex.id).collect(),
        ..Paper::default()
    };
    let project = ProjectState::new_with_paper(pattern, paper);

    let response = validation_snapshot(&project);
    let intersection = response
        .issues
        .iter()
        .find(|issue| issue.code == "boundary_self_intersection")
        .expect("paper self-intersection issue");

    assert_eq!(
        intersection.vertices,
        vec![
            vertices[0].id,
            vertices[1].id,
            vertices[2].id,
            vertices[3].id
        ]
    );
    assert_eq!(
        intersection.edges,
        vec![boundary_edges[0], boundary_edges[2]]
    );
}

#[test]
fn paper_boundary_topology_issues_include_actionable_targets() {
    let sheet = create_rectangular_sheet(20.0, 20.0, false).expect("valid square");
    let (mut pattern, paper) = sheet.into_parts();
    let boundary = paper.boundary_vertices.clone();

    pattern.edges[0].kind = EdgeKind::Mountain;
    let first_duplicate = pattern.edges[1].id;
    let duplicate_edge = Edge {
        id: EdgeId::new(),
        start: pattern.edges[1].end,
        end: pattern.edges[1].start,
        kind: EdgeKind::Boundary,
    };
    let duplicate = duplicate_edge.id;
    pattern.edges.push(duplicate_edge);
    let unexpected_edge = Edge {
        id: EdgeId::new(),
        start: boundary[0],
        end: boundary[2],
        kind: EdgeKind::Boundary,
    };
    let unexpected = unexpected_edge.id;
    pattern.edges.push(unexpected_edge);
    let project = ProjectState::new_with_paper(pattern, paper);

    let response = validation_snapshot(&project);
    let missing = response
        .issues
        .iter()
        .find(|issue| issue.code == "missing_boundary_edge")
        .expect("wrong-kind edge is missing from the Boundary set");
    assert_eq!(missing.vertices, vec![boundary[0], boundary[1]]);
    assert!(missing.edges.is_empty());

    let duplicate_issue = response
        .issues
        .iter()
        .find(|issue| issue.code == "duplicate_boundary_edge")
        .expect("duplicate Boundary record");
    assert_eq!(duplicate_issue.vertices, vec![boundary[1], boundary[2]]);
    assert_eq!(duplicate_issue.edges, vec![first_duplicate, duplicate]);

    let unexpected_issue = response
        .issues
        .iter()
        .find(|issue| issue.code == "unexpected_boundary_edge")
        .expect("unexpected Boundary chord");
    assert_eq!(unexpected_issue.vertices, vec![boundary[0], boundary[2]]);
    assert_eq!(unexpected_issue.edges, vec![unexpected]);
}

#[test]
fn native_save_as_writes_a_loadable_file_and_preserves_editor_history() {
    let directory = TestDirectory::new();
    let selected_path = directory.join("折り紙設計.backup");
    let expected_path = directory.join("折り紙設計.ori2");
    let mut project = unsaved_project_with_redo_history("First project");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let document = project.document();
    let persisted_document = project
        .project_archive()
        .expect("serializable project")
        .document;
    let can_undo = project.editor.can_undo();
    let can_redo = project.editor.can_redo();

    let response = save_project_as_selected_path(
        &mut project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        selected_path,
    )
    .expect("save project under a selected path");

    assert!(!response.canceled);
    assert_eq!(
        project.current_path.as_deref(),
        Some(expected_path.as_path())
    );
    assert_eq!(project.saved_revision, Some(expected_revision));
    assert_eq!(project.saved_document.as_ref(), Some(&document));
    assert!(!project.is_dirty());
    assert_eq!(project.editor.revision(), expected_revision);
    assert_eq!(project.editor.can_undo(), can_undo);
    assert_eq!(project.editor.can_redo(), can_redo);
    assert_eq!(
        load_document_from_path(&expected_path).unwrap(),
        persisted_document
    );
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
}

#[test]
fn native_save_then_reopen_restores_limit_and_both_history_stacks_in_order() {
    let directory = TestDirectory::new();
    let path = directory.join("history-roundtrip.ori2");
    let (mut source, first, second) =
        unsaved_project_with_undo_and_redo_history("History roundtrip");
    let source_project_id = source.project_id;
    let saved_document = source.document();
    let expected_history = source
        .editor
        .export_history_v1(source_project_id)
        .expect("export source history");

    save_project_to_path(&mut source, path.clone()).expect("save history archive");
    assert_eq!(source.editor.history_entry_limit(), 17);
    assert!(source.editor.can_undo());
    assert!(source.editor.can_redo());
    assert_eq!(
        source
            .editor
            .export_history_v1(source_project_id)
            .expect("history remains usable after save"),
        expected_history
    );

    let mut reopened = ProjectState::new(CreasePattern::empty());
    let replaced_instance_id = reopened.instance_id;
    let replaced_project_id = reopened.project_id;
    let loaded = load_project_file(path.clone()).expect("load saved history archive");
    apply_loaded_project_file(
        &mut reopened,
        replaced_instance_id,
        replaced_project_id,
        0,
        loaded,
    )
    .expect("apply saved history archive");

    assert_eq!(reopened.project_id, source_project_id);
    assert_ne!(reopened.instance_id, replaced_instance_id);
    assert_eq!(reopened.current_path.as_deref(), Some(path.as_path()));
    assert_eq!(reopened.saved_revision, Some(0));
    assert_eq!(reopened.saved_document.as_ref(), Some(&saved_document));
    assert!(!reopened.is_dirty());
    assert_eq!(reopened.editor.revision(), 0);
    assert_eq!(reopened.editor.history_entry_limit(), 17);
    assert!(reopened.editor.can_undo());
    assert!(reopened.editor.can_redo());
    assert!(reopened.editor.current_applied_pose().is_none());
    assert_eq!(
        reopened
            .editor
            .export_history_v1(source_project_id)
            .expect("re-export reopened history"),
        expected_history
    );

    reopened.editor.redo(0).expect("redo second command first");
    assert_eq!(
        reopened
            .editor
            .pattern()
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );
    reopened.editor.undo(1).expect("undo second command");
    assert_eq!(reopened.document(), saved_document);
    reopened.editor.undo(2).expect("undo first command");
    assert!(reopened.editor.pattern().vertices.is_empty());
    reopened.editor.redo(3).expect("redo first command first");
    assert_eq!(reopened.editor.pattern().vertices[0].id, first);
    reopened.editor.redo(4).expect("redo second command second");
    assert_eq!(
        reopened
            .editor
            .pattern()
            .vertices
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );
}

#[test]
fn native_open_legacy_two_entry_archive_uses_default_empty_history() {
    let directory = TestDirectory::new();
    let path = directory.join("legacy-two-entry.ori2");
    let document = file_document("Legacy project", 23.0);
    fs::write(
        &path,
        write_project_ori2(&document).expect("write legacy two-entry archive"),
    )
    .expect("persist legacy archive");

    let mut reopened = ProjectState::new(CreasePattern::empty());
    let loaded = load_project_file(path.clone()).expect("load legacy archive");
    let expected_instance_id = reopened.instance_id;
    let expected_project_id = reopened.project_id;
    apply_loaded_project_file(
        &mut reopened,
        expected_instance_id,
        expected_project_id,
        0,
        loaded,
    )
    .expect("apply legacy archive");

    let mut reopened_document = reopened.document();
    assert!(
        reopened_document.thumbnail_svg.is_some(),
        "legacy archives must gain a canonical thumbnail when projected"
    );
    reopened_document.thumbnail_svg = document.thumbnail_svg.clone();
    assert_eq!(reopened_document, document);
    assert_eq!(reopened.editor.revision(), 0);
    assert_eq!(reopened.editor.history_entry_limit(), 128);
    assert!(!reopened.editor.can_undo());
    assert!(!reopened.editor.can_redo());
    assert_eq!(
        reopened
            .project_archive()
            .expect("export canonical legacy state")
            .editor_history,
        None
    );
    assert!(!reopened.is_dirty());
}

#[test]
fn native_save_overwrites_atomically_and_keeps_undo_redo_history() {
    let directory = TestDirectory::new();
    let path = directory.join("overwrite.ori2");
    fs::write(&path, b"pre-existing invalid project").expect("write overwrite sentinel");
    let mut project = unsaved_project_with_redo_history("Overwrite project");

    save_project_to_path(&mut project, path.clone()).expect("replace existing file");
    let first_bytes = fs::read(&path).expect("read first native save");
    let first_persisted_document = project
        .project_archive()
        .expect("serializable project")
        .document;
    assert_ne!(first_bytes, b"pre-existing invalid project");
    assert_eq!(
        load_document_from_path(&path).unwrap(),
        first_persisted_document
    );
    assert!(project.editor.can_redo());

    let revision_before_redo = project.editor.revision();
    project
        .editor
        .redo(revision_before_redo)
        .expect("restore the saved redo command");
    assert!(project.is_dirty());
    let second_persisted_document = project
        .project_archive()
        .expect("serializable edited project")
        .document;
    let revision_before_save = project.editor.revision();
    let can_undo = project.editor.can_undo();
    let can_redo = project.editor.can_redo();

    save_project_to_path(&mut project, path.clone()).expect("overwrite with edited project");
    let second_bytes = fs::read(&path).expect("read overwritten native save");
    assert_ne!(second_bytes, first_bytes);
    assert_eq!(
        load_document_from_path(&path).unwrap(),
        second_persisted_document
    );
    assert_eq!(project.editor.revision(), revision_before_save);
    assert_eq!(project.editor.can_undo(), can_undo);
    assert_eq!(project.editor.can_redo(), can_redo);
    assert!(!project.is_dirty());
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
}

#[cfg(target_os = "windows")]
#[test]
fn windows_staged_save_denies_concurrent_writers_and_cleans_up() {
    let directory = TestDirectory::new();
    let path = directory.join("writer-sharing.ori2");
    let staged = create_staged_file(&path).expect("create protected staged file");

    let writer_error = OpenOptions::new()
        .write(true)
        .open(&staged.path)
        .expect_err("a concurrent writer must be denied while staging");
    let rename_error = fs::rename(&staged.path, directory.join("swapped-stage"))
        .expect_err("a concurrent rename must be denied while staging");

    assert_eq!(writer_error.raw_os_error(), Some(32));
    assert_eq!(rename_error.raw_os_error(), Some(32));
    drop(staged);
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn native_save_overwrite_preserves_unix_file_mode() {
    use std::os::unix::fs::PermissionsExt;

    let directory = TestDirectory::new();
    let path = directory.join("mode-preservation.ori2");
    fs::write(&path, b"pre-existing invalid project").expect("write mode fixture");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("set fixture mode");
    let mut project = unsaved_project_with_redo_history("Mode preservation");

    save_project_to_path(&mut project, path.clone()).expect("overwrite mode fixture");

    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(load_document_from_path(&path).unwrap(), project.document());
}

#[cfg(unix)]
#[test]
fn unix_directory_sync_failure_is_only_reported_before_publish() {
    let directory = TestDirectory::new();
    let path = directory.join("directory-sync.ori2");
    let document = file_document("Directory sync", 42.0);
    let archive = Ori2ProjectArchive::document_only(document.clone());
    let bytes = write_project_ori2(&document).unwrap();

    fs::write(&path, b"keep before failed pre-publish sync").unwrap();
    let mut staged = prepare_staged_file(&path, &archive, &bytes).unwrap();
    let error = commit_unix_staged_project_file(
        &mut staged,
        &path,
        save_path::ExistingDestinationPolicy::ReplaceConfirmed,
        || Err(std::io::Error::other("injected pre-publish sync failure")),
    )
    .expect_err("a pre-publish directory sync failure must abort the commit");
    assert_eq!(error.kind(), std::io::ErrorKind::Other);
    drop(staged);
    assert_eq!(
        fs::read(&path).unwrap(),
        b"keep before failed pre-publish sync"
    );
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);

    let mut staged = prepare_staged_file(&path, &archive, &bytes).unwrap();
    let mut sync_calls = 0_u8;
    commit_unix_staged_project_file(
        &mut staged,
        &path,
        save_path::ExistingDestinationPolicy::ReplaceConfirmed,
        || {
            sync_calls += 1;
            if sync_calls == 1 {
                Ok(())
            } else {
                Err(std::io::Error::other("injected post-publish sync failure"))
            }
        },
    )
    .expect("a post-publish durability failure must not report an ordinary save failure");
    drop(staged);

    assert_eq!(sync_calls, 2);
    assert_eq!(load_document_from_path(&path).unwrap(), document);
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
}

#[test]
fn native_open_replaces_the_project_only_after_loading_and_validation() {
    let directory = TestDirectory::new();
    let path = directory.join("opened.ori2");
    let mut document = file_document("Opened project", 42.0);
    document.paper.cutting_allowed = true;
    persist_document(&path, &document).expect("write open fixture");

    let mut project = unsaved_project_with_redo_history("Replaced project");
    let expected_instance_id = project.instance_id;
    let replaced_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let loaded = load_project_file(path.clone()).expect("load native project");
    let response = apply_loaded_project_file(
        &mut project,
        expected_instance_id,
        replaced_project_id,
        expected_revision,
        loaded,
    )
    .expect("apply validated native project");

    assert!(!response.canceled);
    assert_ne!(project.project_id, replaced_project_id);
    let persisted = project.document();
    assert!(persisted.thumbnail_svg.is_some());
    document.thumbnail_svg = persisted.thumbnail_svg.clone();
    assert_eq!(persisted, document);
    assert_eq!(project.current_path.as_deref(), Some(path.as_path()));
    assert_eq!(project.editor.revision(), 0);
    assert!(!project.editor.can_undo());
    assert!(!project.editor.can_redo());
    assert!(!project.is_dirty());
}

#[test]
fn corrupt_native_open_preserves_project_state_and_history() {
    let directory = TestDirectory::new();
    let secret_name = "private-client-corrupt.ori2";
    let path = directory.join(secret_name);
    let private_payload = b"not an ORIGAMI2 archive: SECRET_PROJECT_CONTENT";
    fs::write(&path, private_payload).expect("write corrupt fixture");
    let project = unsaved_project_with_redo_history("Unaffected project");
    let before = project_state_signature(&project);

    let error = load_project_file(path.clone()).expect_err("corrupt project must fail validation");

    assert_eq!(error, PROJECT_FILE_INVALID_MESSAGE);
    assert!(!error.contains(secret_name));
    assert!(!error.contains("SECRET_PROJECT_CONTENT"));
    assert!(!error.contains(&directory.path.to_string_lossy().into_owned()));
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn corrupt_native_history_open_preserves_every_existing_project_field() {
    let directory = TestDirectory::new();
    let secret_name = "private-client-history-corrupt.ori2";
    let path = directory.join(secret_name);
    let (source, _, _) = unsaved_project_with_undo_and_redo_history("History corruption source");
    persist_project_archive(
        &path,
        &source.project_archive().expect("export source archive"),
    )
    .expect("write valid archive before targeted corruption");
    let corrupt_bytes =
        corrupt_editor_history_payload(fs::read(&path).expect("read valid history archive"));
    fs::write(&path, corrupt_bytes).expect("corrupt only the compressed history payload");

    let (project, _, _) = unsaved_project_with_undo_and_redo_history("Unaffected active project");
    let before = project_state_signature(&project);
    let error = load_project_file(path).expect_err("corrupt editor history must reject the open");

    assert_eq!(error, PROJECT_FILE_INVALID_MESSAGE);
    assert!(!error.contains(secret_name));
    assert!(!error.contains(&directory.path.to_string_lossy().into_owned()));
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn save_rejects_an_invalid_instruction_pose_at_a_reachable_history_endpoint() {
    let directory = TestDirectory::new();
    let path = directory.join("must-not-save-reachable-pose.ori2");
    let mut project = project_with_reachable_invalid_instruction_pose("Unsafe history endpoint");
    let before = project_state_signature(&project);

    let error = save_project_to_path(&mut project, path.clone())
        .expect_err("save must validate every reachable history endpoint");

    assert_eq!(error, PROJECT_SERIALIZATION_FAILED_MESSAGE);
    assert_eq!(project_state_signature(&project), before);
    assert!(!path.exists());
}

#[test]
fn save_rejects_an_invalid_instruction_pose_at_a_redo_endpoint() {
    let directory = TestDirectory::new();
    let path = directory.join("must-not-save-redo-pose.ori2");
    let mut project = project_with_redo_reachable_invalid_instruction_pose("Unsafe Redo endpoint");
    let before = project_state_signature(&project);

    let error = save_project_to_path(&mut project, path.clone())
        .expect_err("save must validate every reachable Redo endpoint");

    assert_eq!(error, PROJECT_SERIALIZATION_FAILED_MESSAGE);
    assert_eq!(project_state_signature(&project), before);
    assert!(!path.exists());
}

#[test]
fn native_open_rejects_reachable_invalid_pose_history_without_mutating_current_state() {
    let directory = TestDirectory::new();
    let secret_name = "private-reachable-pose-history.ori2";
    let path = directory.join(secret_name);
    let source =
        project_with_reachable_invalid_instruction_pose("External unsafe history endpoint");
    let external_archive = Ori2ProjectArchive {
        layer_evidence: None,
        document: source.document(),
        editor_history: Some(
            source
                .editor
                .export_history_v1(source.project_id)
                .expect("export external history fixture"),
        ),
    };
    fs::write(
        &path,
        write_project_archive_ori2(&external_archive)
            .expect("the format boundary accepts replay-consistent external history"),
    )
    .expect("write external history fixture");

    let (active, _, _) = unsaved_project_with_undo_and_redo_history("Unaffected active project");
    let before = project_state_signature(&active);
    let error = load_project_file(path).expect_err("semantic history endpoint must reject open");

    assert_eq!(error, PROJECT_FILE_INVALID_MESSAGE);
    assert!(!error.contains(secret_name));
    assert!(!error.contains("instruction"));
    assert_eq!(project_state_signature(&active), before);
}

#[test]
fn internal_archive_restore_rejects_a_history_bound_to_another_project() {
    let (source, _, _) = unsaved_project_with_undo_and_redo_history("Bound history");
    let mut archive = source.project_archive().expect("export bound history");
    archive.document.project_id = ProjectId::new();

    assert!(restore_archive_editor(&archive).is_err());
}

#[test]
fn native_open_file_failures_use_fixed_path_free_categories() {
    let directory = TestDirectory::new();
    let secret_name = "private-client-missing.ori2";
    let missing_path = directory.join(secret_name);

    let missing_error =
        load_project_file(missing_path).expect_err("missing project must be rejected");
    assert_eq!(missing_error, PROJECT_FILE_OPEN_FAILED_MESSAGE);
    assert!(!missing_error.contains(secret_name));
    assert!(!missing_error.contains(&directory.path.to_string_lossy().into_owned()));
    assert!(!missing_error.to_ascii_lowercase().contains("os error"));

    let oversized_name = "private-client-oversized.ori2";
    let oversized_path = directory.join(oversized_name);
    File::create(&oversized_path)
        .expect("create oversized project fixture")
        .set_len(Ori2Limits::default().max_archive_size + 1)
        .expect("make sparse oversized project fixture");

    let oversized_error =
        load_project_file(oversized_path).expect_err("oversized project must be rejected");
    assert_eq!(oversized_error, PROJECT_FILE_TOO_LARGE_MESSAGE);
    assert!(!oversized_error.contains(oversized_name));
    assert!(!oversized_error.contains(&(Ori2Limits::default().max_archive_size + 1).to_string()));
    assert!(!oversized_error.contains(&directory.path.to_string_lossy().into_owned()));
}

#[test]
fn native_open_instruction_failure_discards_private_semantic_details() {
    let project = initial_project_state();
    let mut document = project.document();
    let private_title = "SECRET_PRIVATE_INSTRUCTION";
    let private_face = FaceId::new();
    document.instruction_timeline.steps.push(InstructionStep {
        id: InstructionStepId::new(),
        title: private_title.to_owned(),
        description: String::new(),
        caution: String::new(),
        duration_ms: 1_000,
        visual: Default::default(),
        pose: InstructionPose {
            model: InstructionPoseModel::AbsoluteHingeAnglesV1,
            source_model_fingerprint: project.editor.fold_model_fingerprint_v1(),
            fixed_face: Some(private_face),
            hinge_angles: Vec::new(),
        },
    });
    let bytes = write_project_ori2(&document)
        .expect("syntactically valid project can carry a semantically invalid pose");
    let directory = TestDirectory::new();
    let secret_name = "private-instruction-project.ori2";
    let path = directory.join(secret_name);
    fs::write(&path, bytes).expect("write instruction failure fixture");

    let error = load_project_file(path).expect_err("semantic instruction failure must be rejected");

    assert_eq!(error, PROJECT_INSTRUCTIONS_INVALID_MESSAGE);
    assert!(!error.contains(private_title));
    assert!(!error.contains(&format!("{private_face:?}")));
    assert!(!error.contains(secret_name));
    assert!(!error.contains(&directory.path.to_string_lossy().into_owned()));
}

#[test]
fn stale_native_open_is_rejected_without_replacing_newer_history() {
    let directory = TestDirectory::new();
    let path = directory.join("stale-open.ori2");
    persist_document(&path, &file_document("Stale open", 17.0)).expect("write stale-open fixture");
    let mut project = unsaved_project_with_redo_history("Active project");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let stale_revision = project.editor.revision();
    let loaded = load_project_file(path).expect("prepare native open");
    execute_command(
        &mut project,
        expected_project_id,
        stale_revision,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(8.0, 9.0),
        },
    )
    .expect("edit while the file dialog is open");
    let before_apply = project_state_signature(&project);

    let error = apply_loaded_project_file(
        &mut project,
        expected_instance_id,
        expected_project_id,
        stale_revision,
        loaded,
    )
    .expect_err("stale open must not replace the active project");

    assert_eq!(error, "the project changed while the file dialog was open");
    assert_eq!(project_state_signature(&project), before_apply);
}

#[test]
fn native_file_dialog_results_cannot_land_after_reopening_the_same_document() {
    let directory = TestDirectory::new();
    let current_path = directory.join("same-document.ori2");
    let opened_path = directory.join("other-document.ori2");
    let selected_path = directory.join("must-not-save.ori2");
    let document = file_document("Same document", 21.0);
    persist_document(&current_path, &document).expect("write same-document fixture");
    persist_document(&opened_path, &file_document("Other document", 34.0))
        .expect("write other-document fixture");

    let mut project = ProjectState::from_valid_document(document.clone(), current_path.clone());
    let stale_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let loaded = load_project_file(opened_path).expect("load delayed open result");

    project = ProjectState::from_valid_document(document, current_path);
    assert_eq!(project.project_id, expected_project_id);
    assert_eq!(project.editor.revision(), expected_revision);
    assert_ne!(project.instance_id, stale_instance_id);
    let before = project_state_signature(&project);

    let open_error = apply_loaded_project_file(
        &mut project,
        stale_instance_id,
        expected_project_id,
        expected_revision,
        loaded,
    )
    .expect_err("a delayed open must not replace a reopened project instance");
    assert_eq!(
        open_error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&project), before);

    let save_error = save_project_as_selected_path(
        &mut project,
        stale_instance_id,
        expected_project_id,
        expected_revision,
        selected_path.clone(),
    )
    .expect_err("a delayed save must not target a reopened project instance");
    assert_eq!(
        save_error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&project), before);
    assert!(!selected_path.exists());
}

#[test]
fn native_save_failure_preserves_state_history_and_existing_target() {
    let directory = TestDirectory::new();
    let occupied_path = directory.join("occupied.ori2");
    fs::create_dir(&occupied_path).expect("create an unreplaceable save target");
    let sentinel = occupied_path.join("keep.txt");
    fs::write(&sentinel, b"keep this directory").expect("write save-failure sentinel");
    let mut project = unsaved_project_with_redo_history("Failed save");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let before = project_state_signature(&project);

    let error = save_project_as_selected_path(
        &mut project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        occupied_path.clone(),
    )
    .expect_err("a directory cannot be replaced by a project file");

    assert_eq!(
        error,
        "プロジェクトを保存先へ安全に確定できなかったため、保存を中止しました。"
    );
    assert!(!error.contains("occupied.ori2"));
    assert!(!error.contains(&directory.path.display().to_string()));
    assert_eq!(project_state_signature(&project), before);
    assert_eq!(fs::read(&sentinel).unwrap(), b"keep this directory");
    assert!(occupied_path.is_dir());
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
}

#[test]
fn stale_native_save_as_is_rejected_before_touching_the_selected_path() {
    let directory = TestDirectory::new();
    let selected_path = directory.join("stale-save");
    let normalized_path = directory.join("stale-save.ori2");
    let mut project = unsaved_project_with_redo_history("Stale save");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let stale_revision = project.editor.revision();
    execute_command(
        &mut project,
        expected_project_id,
        stale_revision,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(99.0, 100.0),
        },
    )
    .expect("edit before stale save-as is applied");
    let before_save = project_state_signature(&project);

    let error = save_project_as_selected_path(
        &mut project,
        expected_instance_id,
        expected_project_id,
        stale_revision,
        selected_path,
    )
    .expect_err("stale save-as must fail");

    assert_eq!(error, "the project changed while the file dialog was open");
    assert_eq!(project_state_signature(&project), before_save);
    assert!(!normalized_path.exists());
}

#[test]
fn native_save_as_cannot_overwrite_an_existing_unconfirmed_corrected_path() {
    let directory = TestDirectory::new();
    let selected_path = directory.join("project.txt");
    let corrected_path = directory.join("project.ori2");
    fs::write(&corrected_path, b"keep existing project").unwrap();
    let mut project = unsaved_project_with_redo_history("Protected project");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let before = project_state_signature(&project);

    let error = save_project_as_selected_path(
        &mut project,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        selected_path.clone(),
    )
    .expect_err("an unconfirmed corrected destination must not be overwritten");

    assert!(error.contains("上書き確認"));
    assert_eq!(project_state_signature(&project), before);
    assert_eq!(fs::read(corrected_path).unwrap(), b"keep existing project");
    assert!(!selected_path.exists());
}

#[test]
fn project_save_target_conversion_error_discards_the_raw_path_and_os_error() {
    let raw_error = r"C:\Users\private-work\secret.ori2: injected operating-system detail";

    let error = project_save_target_conversion_error(raw_error);

    assert_eq!(error, "選択された保存先はローカルファイルではありません。");
    assert!(!error.contains("private-work"));
    assert!(!error.contains("operating-system"));
}

#[test]
fn extension_correction_race_cannot_replace_a_new_destination() {
    let directory = TestDirectory::new();
    let selected_path = directory.join("race-target.backup");
    let corrected_path = directory.join("race-target.ori2");
    let destination =
        ensure_ori2_extension(selected_path).expect("preflight an unused corrected path");
    assert_eq!(
        destination.existing_destination_policy(),
        save_path::ExistingDestinationPolicy::RejectExisting
    );

    let protected_bytes = b"created after extension preflight";
    fs::write(&corrected_path, protected_bytes).unwrap();
    let mut project = unsaved_project_with_redo_history("Race-safe project");
    let before = project_state_signature(&project);

    let error = save_project_to_destination(&mut project, destination)
        .expect_err("atomic create-new commit must reject the intervening destination");

    assert!(error.contains("安全に確定"));
    assert!(!error.contains("race-target"));
    assert_eq!(fs::read(&corrected_path).unwrap(), protected_bytes);
    assert_eq!(project_state_signature(&project), before);
    assert!(
        fs::read_dir(&directory.path)
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| !entry
                .file_name()
                .to_string_lossy()
                .starts_with(".origami2-")),
        "a rejected create-new commit must clean its staged file"
    );
}

#[test]
fn correct_extension_keeps_the_dialog_confirmed_overwrite() {
    let directory = TestDirectory::new();
    let path = directory.join("confirmed.ori2");
    fs::write(&path, b"OS-confirmed old bytes").unwrap();
    let mut project = unsaved_project_with_redo_history("Confirmed overwrite");
    let expected_persisted_document = project
        .project_archive()
        .expect("serializable project")
        .document;
    let destination =
        ensure_ori2_extension(path.clone()).expect("accept a dialog-confirmed extension");

    save_project_to_destination(&mut project, destination)
        .expect("replace the dialog-confirmed destination");

    assert_eq!(
        load_document_from_path(&path).unwrap(),
        expected_persisted_document
    );
    assert_eq!(project.current_path.as_deref(), Some(path.as_path()));
}

#[test]
fn save_as_extension_is_normalized_without_changing_valid_case() {
    assert_eq!(
        ensure_ori2_extension(PathBuf::from("crane")).unwrap(),
        PathBuf::from("crane.ori2")
    );
    assert_eq!(
        ensure_ori2_extension(PathBuf::from("crane.json")).unwrap(),
        PathBuf::from("crane.ori2")
    );
    assert_eq!(
        ensure_ori2_extension(PathBuf::from("crane.ORI2")).unwrap(),
        PathBuf::from("crane.ORI2")
    );
}

#[test]
fn relative_save_path_uses_the_current_directory_for_staging_and_sync() {
    assert_eq!(
        containing_directory(Path::new("bird.ori2")),
        Some(Path::new("."))
    );
    assert_eq!(
        containing_directory(Path::new("projects/bird.ori2")),
        Some(Path::new("projects"))
    );
}

#[cfg(target_os = "windows")]
#[test]
fn windows_relative_save_path_publishes_the_verified_project() {
    let directory = TestDirectory::new_relative();
    let path = directory.join("relative.ori2");
    let document = file_document("Relative Windows save", 31.0);
    assert!(path.is_relative());

    persist_document(&path, &document).expect("publish to a relative Windows path");

    assert_eq!(load_document_from_path(&path).unwrap(), document);
    assert_eq!(fs::read_dir(&directory.path).unwrap().count(), 1);
}

#[test]
fn suggested_name_removes_platform_forbidden_characters() {
    assert_eq!(
        suggested_file_name("  Bird: prototype?  "),
        "Bird_ prototype_.ori2"
    );
    assert_eq!(suggested_file_name("..."), "Untitled.ori2");
}

#[test]
fn generated_container_verification_is_pure_and_checks_identity() {
    let document = ProjectDocument::new("Bird", CreasePattern::empty());
    let archive = Ori2ProjectArchive::document_only(document.clone());
    let bytes = write_project_ori2(&document).expect("generate .ori2");
    verify_generated_ori2(&archive, &bytes).expect("verify generated .ori2");

    let different_document = ProjectDocument::new("Different", CreasePattern::empty());
    let different_archive = Ori2ProjectArchive::document_only(different_document);
    let error = verify_generated_ori2(&different_archive, &bytes)
        .expect_err("a different project must not verify");
    assert_eq!(error, "generated .ori2 data did not round-trip exactly");

    let (history_project, _, _) =
        unsaved_project_with_undo_and_redo_history("History must not disappear");
    let history_archive = history_project
        .project_archive()
        .expect("export nonempty history");
    let document_only_bytes = write_project_ori2(&history_archive.document)
        .expect("write bytes that intentionally omit history");
    let error = verify_generated_ori2(&history_archive, &document_only_bytes)
        .expect_err("stage verification must reject silently dropped history");
    assert_eq!(error, "generated .ori2 data did not round-trip exactly");
}

#[test]
fn document_snapshot_keeps_identity_name_and_dirty_state() {
    let mut document = ProjectDocument::new("Loaded bird", CreasePattern::empty());
    document.memo = "Check the reverse side.".to_owned();
    document.paper.cutting_allowed = true;
    let project = ProjectState::from_valid_document(document.clone(), PathBuf::from("bird.ori2"));
    let response = snapshot(&project);

    assert_eq!(response.project_id, document.project_id);
    assert_eq!(response.name, "Loaded bird");
    assert_eq!(response.memo, "Check the reverse side.");
    assert_eq!(response.current_path.as_deref(), Some("bird.ori2"));
    assert!(!response.is_dirty);
    assert_eq!(response.paper, document.paper);
    assert!(response.cutting_allowed);
    assert!(!response.can_undo);
    let persisted = project.document();
    assert!(persisted.thumbnail_svg.is_some());
    document.thumbnail_svg = persisted.thumbnail_svg.clone();
    assert_eq!(persisted, document);
}

#[test]
fn project_memo_is_dirty_undoable_and_round_trips_through_history() {
    let mut project = ProjectState::new(CreasePattern::empty());
    project
        .editor
        .execute(
            0,
            Command::UpdateProjectMemo {
                memo: "First draft".to_owned(),
            },
        )
        .unwrap();
    assert_eq!(project.document().memo, "First draft");
    assert!(project.is_dirty());

    let archive = project.project_archive().unwrap();
    let mut reopened =
        ProjectState::from_project_archive(archive, PathBuf::from("memo.ori2")).unwrap();
    assert_eq!(reopened.document().memo, "First draft");
    reopened.editor.undo(reopened.editor.revision()).unwrap();
    assert!(reopened.document().memo.is_empty());
    reopened.editor.redo(reopened.editor.revision()).unwrap();
    assert_eq!(reopened.document().memo, "First draft");
}

#[test]
fn stale_project_identity_cannot_mutate_a_replacement_project() {
    let mut project = ProjectState::new(CreasePattern::empty());
    let stale_project_id = ProjectId::new();

    let error = execute_command(
        &mut project,
        stale_project_id,
        0,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(1.0, 1.0),
        },
    )
    .expect_err("a command for another project must fail");

    assert_eq!(
        error,
        "the active project changed before the command was applied"
    );
    assert!(project.editor.pattern().vertices.is_empty());
}

#[test]
fn undoing_to_saved_content_clears_dirty_state() {
    let vertex_id = VertexId::new();
    let document = ProjectDocument::new(
        "Saved bird",
        CreasePattern {
            vertices: vec![Vertex {
                id: vertex_id,
                position: Point2::new(1.0, 2.0),
            }],
            edges: Vec::new(),
        },
    );
    let mut project = ProjectState::from_valid_document(document, PathBuf::from("bird.ori2"));
    let project_id = project.project_id;

    execute_command(
        &mut project,
        project_id,
        0,
        Command::MoveVertex {
            id: vertex_id,
            position: Point2::new(3.0, 4.0),
        },
    )
    .expect("move vertex");
    assert!(project.is_dirty());

    project.editor.undo(1).expect("undo to save point");
    assert!(!project.is_dirty());
}

#[test]
fn undoing_a_removal_to_saved_order_clears_dirty_state() {
    let vertices = [
        Vertex {
            id: VertexId::new(),
            position: Point2::new(0.0, 0.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(1.0, 0.0),
        },
        Vertex {
            id: VertexId::new(),
            position: Point2::new(2.0, 0.0),
        },
    ];
    let document = ProjectDocument::new(
        "Saved bird",
        CreasePattern {
            vertices: vertices.to_vec(),
            edges: Vec::new(),
        },
    );
    let mut project = ProjectState::from_valid_document(document, PathBuf::from("bird.ori2"));
    let project_id = project.project_id;

    execute_command(
        &mut project,
        project_id,
        0,
        Command::RemoveVertex { id: vertices[1].id },
    )
    .expect("remove middle vertex");
    assert!(project.is_dirty());

    project.editor.undo(1).expect("undo to saved ordering");
    assert!(!project.is_dirty());
}

#[test]
fn fold_import_staging_keeps_only_the_latest_preview_and_cancel_is_scoped() {
    let state = FoldImportState::default();
    let project = initial_project_state();
    let first = stage_pending_fold_import(
        &state,
        project.instance_id,
        project.project_id,
        project.editor.revision(),
        br#"{"file_spec":1.2}"#.to_vec(),
    )
    .expect("stage first import");
    let second = stage_pending_fold_import(
        &state,
        project.instance_id,
        project.project_id,
        project.editor.revision(),
        br#"{"file_spec":1.2,"file_title":"newer"}"#.to_vec(),
    )
    .expect("stage replacement import");

    assert_ne!(first, second);
    assert!(pending_fold_import(&state, first, project.project_id, 0).is_err());
    assert_eq!(
        cancel_pending_fold_import(&state, first).unwrap_err(),
        "the FOLD import preview was replaced by a newer preview"
    );
    assert!(pending_fold_import(&state, second, project.project_id, 0).is_ok());
    cancel_pending_fold_import(&state, second).expect("cancel current import");
    cancel_pending_fold_import(&state, second).expect("cancel remains idempotent");
    assert!(lock_fold_import(&state).unwrap().is_none());
}

#[test]
fn svg_import_staging_keeps_only_the_latest_preview_and_cancel_is_scoped() {
    let state = SvgImportState::default();
    let project = initial_project_state();
    let first = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        project.editor.revision(),
        br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.to_vec(),
    )
    .expect("stage first SVG import");
    let second = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        project.editor.revision(),
        br#"<svg xmlns="http://www.w3.org/2000/svg"><title>newer</title></svg>"#.to_vec(),
    )
    .expect("stage replacement SVG import");

    assert_ne!(first, second);
    assert!(pending_svg_import(&state, first, project.project_id, 0).is_err());
    assert_eq!(
        cancel_pending_svg_import(&state, first).unwrap_err(),
        "the SVG import preview was replaced by a newer preview"
    );
    assert!(pending_svg_import(&state, second, project.project_id, 0).is_ok());
    cancel_pending_svg_import(&state, second).expect("cancel current import");
    cancel_pending_svg_import(&state, second).expect("cancel remains idempotent");
    assert!(lock_svg_import(&state).unwrap().pending.is_none());
    assert!(cancel_pending_svg_import(&state, ProjectId::new()).is_err());
}

#[test]
fn svg_import_settings_validation_returns_exact_dimensions_without_replacing_project() {
    let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">
              <rect x="0" y="0" width="100" height="50"
                    fill="none" stroke="#222" data-origami-kind="boundary"/>
              <line x1="0" y1="25" x2="100" y2="25"
                    stroke="#111" data-origami-kind="cut"/>
            </svg>"##;
    let preview = read_svg_preview(bytes).expect("read validation fixture");
    let mut mappings = preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: match group.semantic.as_deref() {
                Some("boundary") => SvgGroupTarget::Boundary,
                Some("cut") => SvgGroupTarget::Cut,
                _ => SvgGroupTarget::Ignore,
            },
        })
        .collect::<Vec<_>>();
    mappings.sort_by_key(|mapping| mapping.group);

    let state = SvgImportState::default();
    let project = initial_project_state();
    let project_before = project_state_signature(&project);
    let import_id = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        project.editor.revision(),
        bytes.to_vec(),
    )
    .expect("stage validation fixture");
    let validation_id = ProjectId::new();
    let pending = begin_svg_import_settings_validation(
        &state,
        validation_id,
        import_id,
        project.project_id,
        project.editor.revision(),
    )
    .expect("begin validation");
    let geometry = validate_svg_import_geometry(&pending.bytes, 2.0, mappings.clone(), None)
        .expect("validate boundary-group geometry");

    let response = {
        let mut slot = lock_svg_import(&state).expect("lock validation state");
        let response = complete_svg_import_settings_validation(
            &mut slot,
            &project,
            SvgImportSettingsValidationCompletion {
                validation: SvgImportSettingsValidation {
                    validation_id,
                    import_id: pending.import_id,
                    expected_instance_id: pending.expected_instance_id,
                    expected_project_id: pending.expected_project_id,
                    expected_revision: pending.expected_revision,
                    millimeters_per_unit_bits: 2.0_f64.to_bits(),
                    boundary_candidate: None,
                    group_mappings: mappings.clone(),
                },
                geometry,
            },
        )
        .expect("complete validation");
        let current = pending_svg_import_in_slot(&slot, import_id, project.project_id, 0).unwrap();
        ensure_svg_import_settings_validation(&slot, current, validation_id, None, 2.0, &mappings)
            .expect("bind validation to exact settings");
        assert!(
            slot.pending.is_some(),
            "validation must retain staged bytes"
        );
        response
    };

    assert_eq!(response.validation_id, validation_id);
    assert_eq!(response.preview_id, import_id);
    assert_eq!(response.expected_project_id, project.project_id);
    assert_eq!(response.expected_revision, 0);
    assert_eq!(response.millimeters_per_unit, 2.0);
    assert_eq!(response.boundary_candidate_id, None);
    assert_eq!(response.width_mm, 200.0);
    assert_eq!(response.height_mm, 100.0);
    assert!(response.has_cuts);
    assert_eq!(project_state_signature(&project), project_before);
}

#[test]
fn svg_import_settings_validation_binds_candidate_and_effective_cut_result() {
    let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">
              <polygon points="0,0 100,0 100,50 0,50"
                       fill="none" stroke="#111" data-origami-kind="cut"/>
            </svg>"##;
    let preview = read_svg_preview(bytes).expect("read candidate fixture");
    let candidate = preview
        .boundary_candidates()
        .iter()
        .find(|candidate| candidate.kind == SvgBoundaryCandidateKind::Polygon)
        .expect("polygon candidate");
    let mappings = preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: SvgGroupTarget::Cut,
        })
        .collect::<Vec<_>>();
    let snapshot =
        svg_import_preview_snapshot(ProjectId::new(), &preview).expect("build candidate snapshot");
    assert!(
        snapshot
            .boundary_candidates
            .iter()
            .any(|candidate| candidate.kind == "polygon")
    );

    let state = SvgImportState::default();
    let project = initial_project_state();
    let import_id = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        0,
        bytes.to_vec(),
    )
    .expect("stage candidate fixture");
    let validation_id = ProjectId::new();
    let pending = begin_svg_import_settings_validation(
        &state,
        validation_id,
        import_id,
        project.project_id,
        0,
    )
    .expect("begin candidate validation");
    let geometry =
        validate_svg_import_geometry(&pending.bytes, 1.0, mappings.clone(), Some(candidate.id))
            .expect("validate selected polygon");
    let response = {
        let mut slot = lock_svg_import(&state).unwrap();
        complete_svg_import_settings_validation(
            &mut slot,
            &project,
            SvgImportSettingsValidationCompletion {
                validation: SvgImportSettingsValidation {
                    validation_id,
                    import_id: pending.import_id,
                    expected_instance_id: pending.expected_instance_id,
                    expected_project_id: pending.expected_project_id,
                    expected_revision: pending.expected_revision,
                    millimeters_per_unit_bits: 1.0_f64.to_bits(),
                    boundary_candidate: Some(candidate.id),
                    group_mappings: mappings,
                },
                geometry,
            },
        )
        .expect("complete candidate validation")
    };

    assert_eq!(response.boundary_candidate_id, Some(candidate.id.0));
    assert_eq!((response.width_mm, response.height_mm), (100.0, 50.0));
    assert!(
        !response.has_cuts,
        "selected source edges become Boundary before effective Cut detection"
    );
}

#[test]
fn svg_import_preview_preserves_every_boundary_candidate_origin() {
    let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"
                              fill="none" stroke="#111">
              <polygon points="0,0 10,0 10,10 0,10"/>
              <polyline points="20,0 30,0 30,10 20,10 20,0"/>
              <rect x="40" y="0" width="10" height="10"/>
              <path d="M 60 0 L 70 0 L 70 10 L 60 10 Z"/>
            </svg>"##;
    let preview = read_svg_preview(bytes).expect("read every candidate origin");
    let snapshot = svg_import_preview_snapshot(ProjectId::new(), &preview)
        .expect("build every candidate origin");
    let kinds = snapshot
        .boundary_candidates
        .iter()
        .map(|candidate| candidate.kind)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        kinds,
        BTreeSet::from([
            "closed_path",
            "polygon",
            "polyline",
            "rectangle",
            "view_box"
        ])
    );
}

#[test]
fn svg_import_settings_validation_rejects_stale_and_superseded_requests() {
    let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg">
              <rect x="0" y="0" width="10" height="20"
                    fill="none" stroke="#222" data-origami-kind="boundary"/>
            </svg>"##;
    let preview = read_svg_preview(bytes).expect("read validation fixture");
    let mappings = preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: SvgGroupTarget::Boundary,
        })
        .collect::<Vec<_>>();
    let state = SvgImportState::default();
    let project = initial_project_state();
    let import_id = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        0,
        bytes.to_vec(),
    )
    .expect("stage validation fixture");

    assert!(
        begin_svg_import_settings_validation(
            &state,
            ProjectId::new(),
            ProjectId::new(),
            project.project_id,
            0,
        )
        .is_err()
    );
    assert!(
        begin_svg_import_settings_validation(
            &state,
            ProjectId::new(),
            import_id,
            project.project_id,
            1,
        )
        .is_err()
    );

    let first_validation_id = ProjectId::new();
    let first = begin_svg_import_settings_validation(
        &state,
        first_validation_id,
        import_id,
        project.project_id,
        0,
    )
    .expect("begin first generation");
    let first_geometry =
        validate_svg_import_geometry(&first.bytes, 1.0, mappings.clone(), None).unwrap();
    let second_validation_id = ProjectId::new();
    let second = begin_svg_import_settings_validation(
        &state,
        second_validation_id,
        import_id,
        project.project_id,
        0,
    )
    .expect("begin second generation");
    {
        let mut slot = lock_svg_import(&state).unwrap();
        assert!(
            complete_svg_import_settings_validation(
                &mut slot,
                &project,
                SvgImportSettingsValidationCompletion {
                    validation: SvgImportSettingsValidation {
                        validation_id: first_validation_id,
                        import_id: first.import_id,
                        expected_instance_id: first.expected_instance_id,
                        expected_project_id: first.expected_project_id,
                        expected_revision: first.expected_revision,
                        millimeters_per_unit_bits: 1.0_f64.to_bits(),
                        boundary_candidate: None,
                        group_mappings: mappings.clone(),
                    },
                    geometry: first_geometry,
                },
            )
            .is_err(),
            "late completion from the old generation must be rejected"
        );
    }
    let second_geometry =
        validate_svg_import_geometry(&second.bytes, 2.0, mappings.clone(), None).unwrap();
    {
        let mut slot = lock_svg_import(&state).unwrap();
        complete_svg_import_settings_validation(
            &mut slot,
            &project,
            SvgImportSettingsValidationCompletion {
                validation: SvgImportSettingsValidation {
                    validation_id: second_validation_id,
                    import_id: second.import_id,
                    expected_instance_id: second.expected_instance_id,
                    expected_project_id: second.expected_project_id,
                    expected_revision: second.expected_revision,
                    millimeters_per_unit_bits: 2.0_f64.to_bits(),
                    boundary_candidate: None,
                    group_mappings: mappings.clone(),
                },
                geometry: second_geometry,
            },
        )
        .expect("complete current generation");
        let pending = pending_svg_import_in_slot(&slot, import_id, project.project_id, 0).unwrap();
        assert!(
            ensure_svg_import_settings_validation(
                &slot,
                pending,
                first_validation_id,
                None,
                2.0,
                &mappings,
            )
            .is_err()
        );
        assert!(
            ensure_svg_import_settings_validation(
                &slot,
                pending,
                second_validation_id,
                None,
                1.0,
                &mappings,
            )
            .is_err(),
            "a changed scale must not reuse old dimensions"
        );
        let mut changed_mappings = mappings.clone();
        changed_mappings[0].target = SvgGroupTarget::Ignore;
        assert!(
            ensure_svg_import_settings_validation(
                &slot,
                pending,
                second_validation_id,
                None,
                2.0,
                &changed_mappings,
            )
            .is_err(),
            "changed mappings must not reuse old dimensions"
        );
    }

    let replacement_id = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        0,
        bytes.to_vec(),
    )
    .expect("stage a newer preview");
    let slot = lock_svg_import(&state).unwrap();
    assert_ne!(replacement_id, import_id);
    assert!(slot.validation.is_none());
    assert!(slot.validation_generation_id.is_none());
}

#[test]
fn svg_import_settings_validation_rejects_a_project_revision_change_without_mutation() {
    let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg">
              <rect x="0" y="0" width="10" height="20"
                    fill="none" stroke="#222" data-origami-kind="boundary"/>
            </svg>"##;
    let preview = read_svg_preview(bytes).expect("read stale revision fixture");
    let mappings = preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: SvgGroupTarget::Boundary,
        })
        .collect::<Vec<_>>();
    let state = SvgImportState::default();
    let mut project = initial_project_state();
    let import_id = stage_pending_svg_import(
        &state,
        project.instance_id,
        project.project_id,
        0,
        bytes.to_vec(),
    )
    .expect("stage stale revision fixture");
    let validation_id = ProjectId::new();
    let pending = begin_svg_import_settings_validation(
        &state,
        validation_id,
        import_id,
        project.project_id,
        0,
    )
    .expect("begin stale revision validation");
    let geometry =
        validate_svg_import_geometry(&pending.bytes, 1.0, mappings.clone(), None).unwrap();
    execute_command(
        &mut project,
        pending.expected_project_id,
        0,
        Command::AddVertex {
            id: VertexId::new(),
            position: Point2::new(12.0, 34.0),
        },
    )
    .expect("change project after validation starts");
    let changed_project = project_state_signature(&project);

    {
        let mut slot = lock_svg_import(&state).unwrap();
        assert!(
            complete_svg_import_settings_validation(
                &mut slot,
                &project,
                SvgImportSettingsValidationCompletion {
                    validation: SvgImportSettingsValidation {
                        validation_id,
                        import_id: pending.import_id,
                        expected_instance_id: pending.expected_instance_id,
                        expected_project_id: pending.expected_project_id,
                        expected_revision: pending.expected_revision,
                        millimeters_per_unit_bits: 1.0_f64.to_bits(),
                        boundary_candidate: None,
                        group_mappings: mappings,
                    },
                    geometry,
                },
            )
            .is_err()
        );
        assert!(slot.validation.is_none());
        assert!(slot.pending.is_some());
    }
    abandon_svg_import_settings_validation(&state, validation_id)
        .expect("clear failed validation generation");
    assert_eq!(project_state_signature(&project), changed_project);
}

#[test]
fn svg_import_settings_validation_rejects_invalid_boundaries_and_mappings() {
    let open = br##"<svg xmlns="http://www.w3.org/2000/svg" stroke="#111">
              <line x1="0" y1="0" x2="10" y2="0" data-origami-kind="boundary"/>
              <line x1="10" y1="0" x2="10" y2="10" data-origami-kind="boundary"/>
              <line x1="10" y1="10" x2="0" y2="10" data-origami-kind="boundary"/>
            </svg>"##;
    let open_preview = read_svg_preview(open).expect("read open boundary");
    let open_mappings = open_preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: SvgGroupTarget::Boundary,
        })
        .collect();
    assert!(validate_svg_import_geometry(open, 1.0, open_mappings, None).is_err());

    let multiple = br##"<svg xmlns="http://www.w3.org/2000/svg" stroke="#111">
              <rect x="0" y="0" width="10" height="10"
                    fill="none" data-origami-kind="boundary"/>
              <rect x="20" y="0" width="10" height="10"
                    fill="none" data-origami-kind="boundary"/>
            </svg>"##;
    let multiple_preview = read_svg_preview(multiple).expect("read multiple boundaries");
    let multiple_mappings = multiple_preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: SvgGroupTarget::Boundary,
        })
        .collect();
    assert!(validate_svg_import_geometry(multiple, 1.0, multiple_mappings, None).is_err());

    let valid = br##"<svg xmlns="http://www.w3.org/2000/svg" stroke="#111">
              <rect x="0" y="0" width="10" height="10"
                    fill="none" data-origami-kind="boundary"/>
              <line x1="0" y1="5" x2="10" y2="5" data-origami-kind="mountain"/>
            </svg>"##;
    let valid_preview = read_svg_preview(valid).expect("read complete mapping fixture");
    let boundary_only = valid_preview
        .style_groups()
        .iter()
        .filter(|group| group.semantic.as_deref() == Some("boundary"))
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: SvgGroupTarget::Boundary,
        })
        .collect();
    assert!(
        validate_svg_import_geometry(valid, 1.0, boundary_only, None).is_err(),
        "every retained style group must be mapped"
    );
    assert!(validate_svg_import_geometry(valid, 0.0, Vec::new(), None).is_err());
}

#[test]
fn svg_import_cancel_rejects_an_applied_token() {
    let state = SvgImportState::default();
    let mut project = initial_project_state();
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let import_id = stage_pending_svg_import(
        &state,
        project.instance_id,
        expected_project_id,
        expected_revision,
        br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.to_vec(),
    )
    .expect("stage SVG import");
    {
        let mut slot = lock_svg_import(&state).expect("lock SVG stage");
        commit_svg_import_replacement(
            &mut project,
            &mut slot.pending,
            import_id,
            expected_project_id,
            expected_revision,
            true,
            create_new_project_state(new_project_parameters()).unwrap(),
        )
        .expect("apply SVG replacement");
    }
    assert!(cancel_pending_svg_import(&state, import_id).is_err());
}

#[test]
fn fold_import_commit_is_an_atomic_new_unsaved_project_replacement() {
    let mut project = unsaved_project_with_redo_history("Existing project");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let import_id = ProjectId::new();
    let mut pending = Some(PendingFoldImport {
        import_id,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        bytes: Arc::from(br#"{"file_spec":1.2}"#.as_slice()),
    });
    let replacement = create_new_project_state(new_project_parameters())
        .expect("create import replacement fixture");
    let replacement_project_id = replacement.project_id;
    let replacement_instance_id = replacement.instance_id;

    let response = commit_fold_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        replacement,
    )
    .expect("commit current import");

    assert_eq!(response.project_id, replacement_project_id);
    assert_eq!(project.instance_id, replacement_instance_id);
    assert_ne!(project.project_id, expected_project_id);
    assert_eq!(project.editor.revision(), 0);
    assert!(!project.editor.can_undo());
    assert!(!project.editor.can_redo());
    assert!(project.current_path.is_none());
    assert!(project.saved_revision.is_none());
    assert!(project.saved_document.is_none());
    assert!(project.is_dirty());
    assert!(pending.is_none());
}

#[test]
fn svg_import_commit_is_an_atomic_new_unsaved_project_replacement() {
    let mut project = unsaved_project_with_redo_history("Existing project");
    let expected_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let import_id = ProjectId::new();
    let mut pending = Some(PendingSvgImport {
        import_id,
        expected_instance_id,
        expected_project_id,
        expected_revision,
        bytes: Arc::from(br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.as_slice()),
    });
    let replacement = create_new_project_state(new_project_parameters())
        .expect("create SVG import replacement fixture");

    let before = project_state_signature(&project);
    let error = commit_svg_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        false,
        replacement,
    )
    .expect_err("dirty SVG replacement must require confirmation");
    assert!(error.contains("explicit confirmation"));
    assert_eq!(project_state_signature(&project), before);
    assert!(pending.is_some());

    let replacement = create_new_project_state(new_project_parameters())
        .expect("create confirmed SVG import replacement fixture");
    let replacement_project_id = replacement.project_id;
    let replacement_instance_id = replacement.instance_id;
    let response = commit_svg_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        true,
        replacement,
    )
    .expect("commit current SVG import");

    assert_eq!(response.project_id, replacement_project_id);
    assert_eq!(project.instance_id, replacement_instance_id);
    assert_ne!(project.project_id, expected_project_id);
    assert_eq!(project.editor.revision(), 0);
    assert!(!project.editor.can_undo());
    assert!(!project.editor.can_redo());
    assert!(project.current_path.is_none());
    assert!(project.saved_revision.is_none());
    assert!(project.saved_document.is_none());
    assert!(project.is_dirty());
    assert!(pending.is_none());
}

#[test]
fn svg_import_commit_rejects_revision_and_instance_aba_without_mutation() {
    let mut project = unsaved_project_with_redo_history("Existing project");
    let stale_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let import_id = ProjectId::new();
    let pending_template = PendingSvgImport {
        import_id,
        expected_instance_id: stale_instance_id,
        expected_project_id,
        expected_revision,
        bytes: Arc::from(br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#.as_slice()),
    };

    project
        .editor
        .execute(
            expected_revision,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(12.0, 13.0),
            },
        )
        .expect("edit after SVG preview");
    let revision_before = project_state_signature(&project);
    let mut pending = Some(pending_template.clone());
    let error = commit_svg_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        true,
        create_new_project_state(new_project_parameters()).unwrap(),
    )
    .expect_err("stale SVG revision must fail");
    assert_eq!(error, "the project changed while the file dialog was open");
    assert_eq!(project_state_signature(&project), revision_before);
    assert!(pending.is_some());

    let document = project.document();
    project = ProjectState::from_valid_document(document, PathBuf::from("same.ori2"));
    project.project_id = expected_project_id;
    assert_ne!(project.instance_id, stale_instance_id);
    let instance_before = project_state_signature(&project);
    let mut pending = Some(pending_template);
    let error = commit_svg_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        true,
        create_new_project_state(new_project_parameters()).unwrap(),
    )
    .expect_err("reopened project instance must fail");
    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&project), instance_before);
    assert!(pending.is_some());
}

#[test]
fn fold_import_commit_rejects_revision_and_instance_aba_without_mutation() {
    let mut project = unsaved_project_with_redo_history("Existing project");
    let stale_instance_id = project.instance_id;
    let expected_project_id = project.project_id;
    let expected_revision = project.editor.revision();
    let import_id = ProjectId::new();
    let pending_template = PendingFoldImport {
        import_id,
        expected_instance_id: stale_instance_id,
        expected_project_id,
        expected_revision,
        bytes: Arc::from(br#"{"file_spec":1.2}"#.as_slice()),
    };

    project
        .editor
        .execute(
            expected_revision,
            Command::AddVertex {
                id: VertexId::new(),
                position: Point2::new(12.0, 13.0),
            },
        )
        .expect("edit after preview");
    let revision_before = project_state_signature(&project);
    let mut pending = Some(pending_template.clone());
    let error = commit_fold_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        create_new_project_state(new_project_parameters()).unwrap(),
    )
    .expect_err("stale revision must fail");
    assert_eq!(error, "the project changed while the file dialog was open");
    assert_eq!(project_state_signature(&project), revision_before);
    assert!(pending.is_some());

    let document = project.document();
    project = ProjectState::from_valid_document(document, PathBuf::from("same.ori2"));
    project.project_id = expected_project_id;
    assert_ne!(project.instance_id, stale_instance_id);
    let instance_before = project_state_signature(&project);
    let mut pending = Some(pending_template);
    let error = commit_fold_import_replacement(
        &mut project,
        &mut pending,
        import_id,
        expected_project_id,
        expected_revision,
        create_new_project_state(new_project_parameters()).unwrap(),
    )
    .expect_err("reopened project instance must fail");
    assert_eq!(
        error,
        "the open project instance changed while the file dialog was open"
    );
    assert_eq!(project_state_signature(&project), instance_before);
    assert!(pending.is_some());
}

#[test]
fn fold_import_mapping_and_scale_validation_reject_ambiguous_requests() {
    assert!(validate_import_scale(1.0).is_ok());
    for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY, 1_000_000_000.000_001] {
        assert!(validate_import_scale(invalid).is_err());
    }

    let valid = validate_fold_import_mapping_requests(vec![
        FoldImportAssignmentMappingRequest {
            source: "M".to_owned(),
            target: FoldImportTargetRequest::Mountain,
        },
        FoldImportAssignmentMappingRequest {
            source: "U".to_owned(),
            target: FoldImportTargetRequest::Valley,
        },
        FoldImportAssignmentMappingRequest {
            source: "J".to_owned(),
            target: FoldImportTargetRequest::Ignore,
        },
    ])
    .expect("validate supported mappings");
    assert_eq!(valid.len(), 3);

    assert!(
        validate_fold_import_mapping_requests(vec![FoldImportAssignmentMappingRequest {
            source: "M".to_owned(),
            target: FoldImportTargetRequest::Valley,
        }])
        .is_err()
    );
    assert!(
        validate_fold_import_mapping_requests(vec![FoldImportAssignmentMappingRequest {
            source: "X".to_owned(),
            target: FoldImportTargetRequest::Ignore,
        }])
        .is_err()
    );
    assert!(
        validate_fold_import_mapping_requests(vec![
            FoldImportAssignmentMappingRequest {
                source: "F".to_owned(),
                target: FoldImportTargetRequest::Auxiliary,
            },
            FoldImportAssignmentMappingRequest {
                source: "F".to_owned(),
                target: FoldImportTargetRequest::Ignore,
            },
        ])
        .is_err()
    );
}

#[test]
fn fold_import_mapping_or_geometry_failure_preserves_project_and_pending_preview() {
    let project = unsaved_project_with_redo_history("Keep this project");
    let before = project_state_signature(&project);
    let valid_bytes = serde_json::to_vec(&serde_json::json!({
        "file_spec": 1.2,
        "frame_unit": "mm",
        "vertices_coords": [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
        "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0], [0, 2]],
        "edges_assignment": ["B", "B", "B", "B", "M"]
    }))
    .expect("serialize mapping fixture");
    let import_id = ProjectId::new();
    let mut pending = Some(PendingFoldImport {
        import_id,
        expected_instance_id: project.instance_id,
        expected_project_id: project.project_id,
        expected_revision: project.editor.revision(),
        bytes: Arc::from(valid_bytes.clone()),
    });

    let mapping_error = build_fold_import_replacement(
        &valid_bytes,
        "Missing mapping".to_owned(),
        1.0,
        FoldBoundaryCandidateId(0),
        HashMap::new(),
    )
    .err()
    .expect("missing M mapping must fail");
    assert!(mapping_error.contains("no mapping was selected"));
    assert_eq!(project_state_signature(&project), before);
    assert_eq!(
        pending.as_ref().map(|value| value.import_id),
        Some(import_id)
    );

    let crossing_bytes = serde_json::to_vec(&serde_json::json!({
        "file_spec": 1.2,
        "frame_unit": "mm",
        "vertices_coords": [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
        "edges_vertices": [
            [0, 1], [1, 2], [2, 3], [3, 0],
            [0, 2], [1, 3]
        ],
        "edges_assignment": ["B", "B", "B", "B", "M", "V"]
    }))
    .expect("serialize crossing fixture");
    let geometry_error = build_fold_import_replacement(
        &crossing_bytes,
        "Crossing".to_owned(),
        1.0,
        FoldBoundaryCandidateId(0),
        HashMap::from([
            ("M".to_owned(), FoldImportTargetRequest::Mountain),
            ("V".to_owned(), FoldImportTargetRequest::Valley),
        ]),
    )
    .err()
    .expect("unsplit crossing must fail final validation");
    assert!(geometry_error.contains("validation issue(s)"));
    assert_eq!(project_state_signature(&project), before);
    assert_eq!(
        pending.as_ref().map(|value| value.import_id),
        Some(import_id)
    );

    let replacement =
        create_new_project_state(new_project_parameters()).expect("create unused replacement");
    // The failed conversion path never reaches the only replacement
    // boundary; retaining this assertion guards accidental future calls.
    assert_ne!(replacement.project_id, project.project_id);
    assert!(pending.take().is_some());
    assert_eq!(project_state_signature(&project), before);
}

#[test]
fn fold_import_file_errors_do_not_expose_the_selected_path() {
    let directory = TestDirectory::new();
    let secret_name = "private-client-design.fold";
    let path = directory.join(secret_name);

    let missing_error = read_fold_import_bytes(&path).expect_err("missing import must be rejected");
    assert_eq!(missing_error, FOLD_FILE_OPEN_FAILED_MESSAGE);
    assert!(!missing_error.contains(secret_name));
    assert!(!missing_error.contains(&directory.path.to_string_lossy().into_owned()));
    assert!(!missing_error.to_ascii_lowercase().contains("os error"));

    let private_file_spec = 987_654_321.125_f64;
    let private_value = private_file_spec.to_string();
    let malformed = serde_json::to_vec(&serde_json::json!({
        "file_spec": private_file_spec,
        "vertices_coords": [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0]],
        "edges_assignment": ["B", "B", "B", "B"]
    }))
    .expect("serialize private malformed FOLD fixture");
    fs::write(&path, &malformed).expect("write malformed FOLD fixture");
    let malformed_error =
        load_fold_import_preview(&path).expect_err("unsupported FOLD version must be rejected");
    assert_eq!(malformed_error, FOLD_FILE_INVALID_MESSAGE);
    assert!(!malformed_error.contains(&private_value));
    assert!(!malformed_error.contains(secret_name));

    let staged_error = build_fold_import_replacement(
        &malformed,
        "Private staged input".to_owned(),
        1.0,
        FoldBoundaryCandidateId(0),
        HashMap::new(),
    )
    .err()
    .expect("staged unsupported FOLD version must be rejected");
    assert_eq!(staged_error, FOLD_FILE_INVALID_MESSAGE);
    assert!(!staged_error.contains(&private_value));

    File::create(&path)
        .expect("create oversized fixture")
        .set_len(MAX_FOLD_IMPORT_FILE_SIZE + 1)
        .expect("make sparse oversized fixture");
    let oversized_error =
        read_fold_import_bytes(&path).expect_err("oversized import must be rejected");
    assert_eq!(oversized_error, FOLD_FILE_TOO_LARGE_MESSAGE);
    assert!(!oversized_error.contains(secret_name));
    assert!(!oversized_error.contains(&directory.path.to_string_lossy().into_owned()));
    assert!(!oversized_error.contains(&(MAX_FOLD_IMPORT_FILE_SIZE + 1).to_string()));
}

#[test]
fn fold_import_preview_contract_and_conversion_create_a_valid_editable_project() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "file_spec": 1.2,
        "file_title": "  取込テスト  ",
        "frame_unit": "cm",
        "vertices_coords": [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
        "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0], [0, 2]],
        "edges_assignment": ["B", "B", "B", "B", "M"]
    }))
    .expect("serialize FOLD fixture");
    let preview = read_fold_preview(&bytes).expect("read FOLD preview");
    let import_id = ProjectId::new();
    let response = fold_import_preview_snapshot(import_id, &preview);

    assert_eq!(response.import_id, import_id);
    assert_eq!(response.file_name, FOLD_IMPORT_FILE_LABEL);
    assert_eq!(response.suggested_name, "取込テスト");
    assert_eq!(response.file_spec.as_deref(), Some("1.2"));
    assert_eq!(response.frame_unit.as_deref(), Some("cm"));
    assert_eq!(response.default_mm_per_unit, Some(10.0));
    assert_eq!(response.vertex_count, 4);
    assert_eq!(response.edge_count, 5);
    assert_eq!(response.boundary_edge_count, 4);
    assert_eq!(response.fixed_boundary_candidate_id, Some(0));
    assert_eq!(
        response.boundary_candidates,
        vec![FoldImportBoundaryCandidateSnapshot {
            id: 0,
            source: "assigned_boundary",
            edge_indices: vec![0, 1, 2, 3],
        }]
    );
    assert_eq!(
        response.assignments,
        vec![
            FoldImportAssignmentSummary {
                assignment: "B".to_owned(),
                count: 4,
            },
            FoldImportAssignmentSummary {
                assignment: "M".to_owned(),
                count: 1,
            },
        ]
    );
    assert_eq!(response.preview_vertices.len(), 4);
    assert_eq!(response.preview_edges.len(), 5);
    assert_eq!(
        response
            .preview_edges
            .iter()
            .map(|edge| edge.source_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
    assert!(!response.preview_truncated);
    assert!(response.warnings.is_empty());

    let replacement = build_fold_import_replacement(
        &bytes,
        "取込テスト".to_owned(),
        10.0,
        FoldBoundaryCandidateId(0),
        HashMap::from([("M".to_owned(), FoldImportTargetRequest::Mountain)]),
    )
    .expect("convert FOLD into a project");
    assert_eq!(replacement.name, "取込テスト");
    assert_eq!(replacement.editor.pattern().vertices.len(), 4);
    assert_eq!(replacement.editor.pattern().edges.len(), 5);
    assert_eq!(replacement.editor.paper().boundary_vertices.len(), 4);
    assert!(
        replacement
            .editor
            .pattern()
            .vertices
            .iter()
            .any(|vertex| vertex.position == Point2::new(20.0, 20.0))
    );
    assert_eq!(
        replacement
            .editor
            .pattern()
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Mountain)
            .count(),
        1
    );
    assert!(!replacement.editor.paper().cutting_allowed);
    assert!(replacement.editor.instruction_timeline().steps.is_empty());
    assert_eq!(replacement.editor.revision(), 0);
    assert!(!replacement.editor.can_undo());
    assert!(!replacement.editor.can_redo());
    assert!(replacement.current_path.is_none());
    assert!(replacement.saved_document.is_none());
    assert!(replacement.is_dirty());
}

#[test]
fn fold_import_requires_and_revalidates_an_inferred_boundary_choice() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "file_spec": 1.2,
        "frame_unit": "mm",
        "vertices_coords": [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
        "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0], [0, 2]]
    }))
    .expect("serialize assignment-free FOLD fixture");
    let preview = read_fold_preview(&bytes).expect("read assignment-free FOLD preview");
    let response = fold_import_preview_snapshot(ProjectId::new(), &preview);

    assert_eq!(response.fixed_boundary_candidate_id, None);
    assert_eq!(response.boundary_candidates.len(), 1);
    let candidate = &response.boundary_candidates[0];
    assert_eq!(candidate.source, "inferred_outer_face");
    assert_eq!(candidate.edge_indices, vec![0, 1, 2, 3]);
    assert!(
        response
            .preview_edges
            .iter()
            .filter(|edge| candidate.edge_indices.contains(&edge.source_index))
            .all(|edge| edge.assignment == "U")
    );

    let replacement = build_fold_import_replacement(
        &bytes,
        "外周候補を選択".to_owned(),
        1.0,
        FoldBoundaryCandidateId(candidate.id),
        HashMap::from([("U".to_owned(), FoldImportTargetRequest::Auxiliary)]),
    )
    .expect("convert with the explicitly selected candidate");
    assert_eq!(replacement.editor.paper().boundary_vertices.len(), 4);
    assert_eq!(
        replacement
            .editor
            .pattern()
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Boundary)
            .count(),
        4
    );
    assert_eq!(
        replacement
            .editor
            .pattern()
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Auxiliary)
            .count(),
        1
    );

    let stale_error = match build_fold_import_replacement(
        &bytes,
        "存在しない候補".to_owned(),
        1.0,
        FoldBoundaryCandidateId(candidate.id.saturating_add(1)),
        HashMap::from([("U".to_owned(), FoldImportTargetRequest::Auxiliary)]),
    ) {
        Ok(_) => panic!("an absent candidate ID must be rejected after reparsing"),
        Err(error) => error,
    };
    assert!(stale_error.contains("is not present in this preview"));
}

#[test]
fn fold_import_rejects_an_active_edge_outside_the_paper() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "file_spec": 1.2,
        "frame_unit": "mm",
        "vertices_coords": [
            [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0],
            [2.0, 0.0], [2.0, 1.0]
        ],
        "edges_vertices": [[0, 1], [1, 2], [2, 3], [3, 0], [4, 5]],
        "edges_assignment": ["B", "B", "B", "B", "M"]
    }))
    .expect("serialize outside-edge fixture");

    let error = build_fold_import_replacement(
        &bytes,
        "紙外の折り線".to_owned(),
        1.0,
        FoldBoundaryCandidateId(0),
        HashMap::from([("M".to_owned(), FoldImportTargetRequest::Mountain)]),
    )
    .err()
    .expect("an active edge outside the paper must be rejected");

    assert!(error.contains("active edge(s) outside the paper boundary"));
}

#[test]
fn fold_import_applies_valley_cut_and_ignore_mapping_with_scale() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "file_spec": 1.2,
        "frame_unit": "unit",
        "vertices_coords": [
            [0.0, 0.0], [2.0, 0.0], [4.0, 0.0],
            [4.0, 4.0], [2.0, 4.0], [0.0, 4.0]
        ],
        "edges_vertices": [
            [0, 1], [1, 2], [2, 3], [3, 4], [4, 5], [5, 0],
            [0, 3], [0, 4], [1, 3], [2, 5]
        ],
        "edges_assignment": ["B", "B", "B", "B", "B", "B", "M", "V", "C", "F"]
    }))
    .expect("serialize mapped FOLD fixture");
    let replacement = build_fold_import_replacement(
        &bytes,
        "複数線種".to_owned(),
        2.5,
        FoldBoundaryCandidateId(0),
        HashMap::from([
            ("M".to_owned(), FoldImportTargetRequest::Mountain),
            ("V".to_owned(), FoldImportTargetRequest::Valley),
            ("C".to_owned(), FoldImportTargetRequest::Cut),
            ("F".to_owned(), FoldImportTargetRequest::Ignore),
        ]),
    )
    .expect("convert explicit mapped assignments");
    let edges = &replacement.editor.pattern().edges;

    assert_eq!(edges.len(), 9);
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Boundary)
            .count(),
        6
    );
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Mountain)
            .count(),
        1
    );
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Valley)
            .count(),
        1
    );
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Cut)
            .count(),
        1
    );
    assert!(replacement.editor.paper().cutting_allowed);
    assert!(
        replacement
            .editor
            .pattern()
            .vertices
            .iter()
            .any(|vertex| vertex.position == Point2::new(10.0, 10.0))
    );
}

#[test]
fn fold_import_preview_truncation_remaps_every_rendered_endpoint() {
    let interior_edge_count = MAX_FOLD_IMPORT_PREVIEW_EDGES - 3;
    let mut vertices = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let mut edges = Vec::new();
    let mut assignments = Vec::new();
    for index in 0..interior_edge_count {
        let x = 10.0 + index as f64;
        let start = vertices.len();
        vertices.push([x, 2.0]);
        vertices.push([x, 3.0]);
        edges.push([start, start + 1]);
        assignments.push("F");
    }
    edges.extend([[0_usize, 1_usize], [1, 2], [2, 3], [3, 0]]);
    assignments.extend(["B"; 4]);
    let bytes = serde_json::to_vec(&serde_json::json!({
        "vertices_coords": vertices,
        "edges_vertices": edges,
        "edges_assignment": assignments,
        "file_classes": ["singleModel"]
    }))
    .expect("serialize large preview fixture");
    let preview = read_fold_preview(&bytes).expect("read large preview");
    let response = fold_import_preview_snapshot(ProjectId::new(), &preview);

    assert!(response.preview_truncated);
    assert_eq!(response.preview_edges.len(), MAX_FOLD_IMPORT_PREVIEW_EDGES);
    assert!(response.preview_vertices.len() < response.vertex_count);
    assert!(response.preview_edges.iter().all(|edge| {
        edge.start < response.preview_vertices.len() && edge.end < response.preview_vertices.len()
    }));
    assert_eq!(
        response
            .preview_edges
            .iter()
            .filter(|edge| edge.assignment == "B")
            .count(),
        4
    );
    assert_eq!(
        response
            .assignments
            .iter()
            .map(|summary| summary.assignment.as_str())
            .collect::<Vec<_>>(),
        vec!["B", "F"]
    );
    assert!(response.warnings.iter().all(|warning| !warning.is_ascii()));
    assert!(
        response
            .warnings
            .iter()
            .any(|warning| warning.contains("ファイル分類"))
    );
}

#[test]
fn svg_import_file_errors_do_not_expose_the_selected_path() {
    let directory = TestDirectory::new();
    let secret_name = "private-client-design.svg";
    let path = directory.join(secret_name);

    let missing_error =
        read_svg_import_bytes(&path).expect_err("missing SVG import must be rejected");
    assert_eq!(missing_error, SVG_FILE_OPEN_FAILED_MESSAGE);
    assert!(!missing_error.contains(secret_name));
    assert!(!missing_error.contains(&directory.path.to_string_lossy().into_owned()));
    assert!(!missing_error.to_ascii_lowercase().contains("os error"));

    fs::write(
        &path,
        br#"<svg xmlns="http://www.w3.org/2000/svg"><SECRET_MARKER></OTHER_SECRET></svg>"#,
    )
    .expect("write malformed SVG fixture");
    let malformed_error =
        load_svg_import_preview(&path).expect_err("malformed SVG import must be rejected");
    assert_eq!(malformed_error, SVG_FILE_INVALID_MESSAGE);
    assert!(!malformed_error.contains("SECRET_MARKER"));
    assert!(!malformed_error.contains("OTHER_SECRET"));
    assert!(!malformed_error.contains(secret_name));

    File::create(&path)
        .expect("create oversized SVG fixture")
        .set_len(MAX_SVG_IMPORT_FILE_SIZE + 1)
        .expect("make sparse oversized SVG fixture");
    let oversized_error =
        read_svg_import_bytes(&path).expect_err("oversized SVG import must be rejected");
    assert_eq!(oversized_error, SVG_FILE_TOO_LARGE_MESSAGE);
    assert!(!oversized_error.contains(secret_name));
    assert!(!oversized_error.contains(&directory.path.to_string_lossy().into_owned()));
    assert!(!oversized_error.contains(&(MAX_SVG_IMPORT_FILE_SIZE + 1).to_string()));
}

#[test]
fn svg_import_warning_messages_do_not_echo_source_style_values() {
    for kind in [
        SvgWarningKind::UnsupportedCssSelector("#SECRET_SELECTOR".to_owned()),
        SvgWarningKind::UnsupportedPaint("url(SECRET_PAINT)".to_owned()),
        SvgWarningKind::UnsupportedLengthUnit("SECRET_LENGTH".to_owned()),
    ] {
        let message = svg_import_warning_message(&SvgPreviewWarning {
            kind,
            occurrences: 1,
        });
        assert!(!message.contains("SECRET"));
    }

    let source = br##"<svg xmlns="http://www.w3.org/2000/svg"
                              viewBox="0 0 10 10" width="10mm" height="10mm"
                              fill="none">
              <line stroke="#111111" stroke-linecap="SECRET_LINE_CAP"
                    x1="0" y1="0" x2="10" y2="10"/>
            </svg>"##;
    let preview = read_svg_preview(source).expect("parse unknown line-cap fixture");
    assert_eq!(
        preview.warnings(),
        &[SvgPreviewWarning {
            kind: SvgWarningKind::UnsupportedAttribute("stroke-linecap".to_owned()),
            occurrences: 1,
        }]
    );
    let response = svg_import_preview_snapshot(ProjectId::new(), &preview)
        .expect("build unknown line-cap snapshot");
    let encoded = serde_json::to_string(&response).expect("serialize SVG preview snapshot");
    assert!(!encoded.contains("SECRET"));
    assert!(!encoded.contains("LINE_CAP"));
}

#[test]
fn svg_import_preview_contract_and_conversion_create_a_valid_editable_project() {
    let source = r##"<?xml version="1.0" encoding="UTF-8"?>
            <svg xmlns="http://www.w3.org/2000/svg"
                 viewBox="0 0 100 100" width="100mm" height="100mm">
              <title>  SVG取込テスト  </title>
              <rect x="0" y="0" width="100" height="100"
                    fill="none" stroke="#222222" data-origami-kind="boundary"/>
              <line id="main-fold" x1="0" y1="0" x2="100" y2="100"
                    stroke="#cc3344" stroke-linecap="round"
                    data-origami-kind="mountain"/>
            </svg>"##;
    let bytes = source.as_bytes();
    let preview = read_svg_preview(bytes).expect("read SVG preview");
    let import_id = ProjectId::new();
    let response =
        svg_import_preview_snapshot(import_id, &preview).expect("build bounded SVG preview");

    assert_eq!(response.import_id, import_id);
    assert_eq!(response.file_name, SVG_IMPORT_FILE_LABEL);
    assert_eq!(response.suggested_name, "SVG取込テスト");
    assert_eq!(response.default_mm_per_unit, Some(1.0));
    assert_eq!(
        response.root_view_box,
        Some(SvgRootViewBox {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        })
    );
    assert_eq!(response.root_physical_size.width_millimetres, Some(100.0));
    assert_eq!(response.root_physical_size.height_millimetres, Some(100.0));
    assert_eq!(response.source_segment_count, 5);
    assert_eq!(response.style_groups.len(), 2);
    assert!(response.style_groups.iter().all(|group| {
        group.element_count > 0
            && group.segment_count > 0
            && matches!(
                group.line_cap,
                SvgLineCap::Butt | SvgLineCap::Round | SvgLineCap::Square
            )
            && group
                .stroke_color
                .as_deref()
                .is_some_and(|color| color.starts_with('#'))
    }));
    let main_fold_group = response
        .style_groups
        .iter()
        .find(|group| group.representative_id.as_deref() == Some("main-fold"))
        .expect("main fold style group");
    assert_eq!(main_fold_group.element_count, 1);
    assert_eq!(main_fold_group.segment_count, 1);
    assert_eq!(main_fold_group.line_cap, SvgLineCap::Round);
    assert_eq!(
        serde_json::to_value(main_fold_group)
            .expect("serialize SVG style group snapshot")
            .get("line_cap")
            .and_then(serde_json::Value::as_str),
        Some("round")
    );
    assert_eq!(response.preview_edges.len(), 5);
    assert!(!response.preview_truncated);
    assert!(response.preview_edges.iter().all(|edge| {
        edge.start < response.preview_vertices.len() && edge.end < response.preview_vertices.len()
    }));
    assert!(
        response
            .boundary_candidates
            .iter()
            .any(|candidate| candidate.kind == "view_box")
    );
    assert!(
        response
            .boundary_candidates
            .iter()
            .any(|candidate| candidate.kind == "rectangle")
    );
    assert!(response.boundary_candidates.iter().all(|candidate| {
        candidate.segment_count == candidate.vertices.len() && candidate.segment_count >= 3
    }));
    assert!(
        response
            .warnings
            .iter()
            .any(|warning| warning.contains("data-origami-kind"))
    );

    let rectangle = preview
        .boundary_candidates()
        .iter()
        .find(|candidate| candidate.kind == SvgBoundaryCandidateKind::Rectangle)
        .expect("rectangle boundary candidate");
    let mappings: Vec<SvgGroupMapping> = preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: match group.semantic.as_deref() {
                Some("mountain") => SvgGroupTarget::Mountain,
                _ => SvgGroupTarget::Ignore,
            },
        })
        .collect();
    let boundary_error = build_svg_import_replacement(
        bytes,
        SvgImportReplacementOptions {
            name: "SVG取込テスト".to_owned(),
            millimeters_per_unit: 1.0,
            group_mappings: mappings.clone(),
            boundary_candidate: Some(rectangle.id),
            boundary_confirmed: false,
            warnings_acknowledged: true,
            cutting_allowed_confirmed: false,
        },
    )
    .err()
    .expect("boundary must require explicit confirmation");
    assert!(boundary_error.contains("boundary must be explicitly confirmed"));
    let warning_error = build_svg_import_replacement(
        bytes,
        SvgImportReplacementOptions {
            name: "SVG取込テスト".to_owned(),
            millimeters_per_unit: 1.0,
            group_mappings: mappings.clone(),
            boundary_candidate: Some(rectangle.id),
            boundary_confirmed: true,
            warnings_acknowledged: false,
            cutting_allowed_confirmed: false,
        },
    )
    .err()
    .expect("warnings must require explicit confirmation");
    assert!(warning_error.contains("warnings must be explicitly acknowledged"));
    let replacement = build_svg_import_replacement(
        bytes,
        SvgImportReplacementOptions {
            name: "SVG取込テスト".to_owned(),
            millimeters_per_unit: 1.0,
            group_mappings: mappings,
            boundary_candidate: Some(rectangle.id),
            boundary_confirmed: true,
            warnings_acknowledged: true,
            cutting_allowed_confirmed: false,
        },
    )
    .expect("convert SVG into a project");

    assert_eq!(replacement.name, "SVG取込テスト");
    assert_eq!(replacement.editor.pattern().edges.len(), 5);
    assert_eq!(replacement.editor.paper().boundary_vertices.len(), 4);
    assert_eq!(
        replacement
            .editor
            .pattern()
            .edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Mountain)
            .count(),
        1
    );
    assert!(!replacement.editor.paper().cutting_allowed);
    assert!(replacement.editor.instruction_timeline().steps.is_empty());
    assert_eq!(replacement.editor.revision(), 0);
    assert!(!replacement.editor.can_undo());
    assert!(!replacement.editor.can_redo());
    assert!(replacement.current_path.is_none());
    assert!(replacement.saved_document.is_none());
    assert!(replacement.is_dirty());
}

#[test]
fn svg_import_preview_rejects_more_than_sixty_four_warning_categories() {
    let mut source = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"
                     width="100mm" height="100mm" fill="none" stroke="#111">
                   <title>{}</title>"##,
        "a".repeat(MAX_PROJECT_NAME_CHARS + 1)
    );
    for index in 0..63 {
        let class = if index == 0 { r#" class="fold""# } else { "" };
        source.push_str(&format!(
            r#"<line{class} unsupported{index}="x" x1="0" y1="{index}" x2="1" y2="{index}"/>"#
        ));
    }
    source.push_str("</svg>");

    let preview = read_svg_preview(source.as_bytes()).expect("bounded warning fixture");
    assert_eq!(preview.warnings().len(), 63);
    let error = svg_import_preview_snapshot(ProjectId::new(), &preview)
        .expect_err("synthetic warning categories must not be truncated");
    assert!(error.contains("more than 64"));
}

#[test]
fn svg_cut_mapping_requires_explicit_permission_and_splits_crossings() {
    let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
              <rect x="0" y="0" width="100" height="100"
                    fill="none" stroke="#222" data-origami-kind="boundary"/>
              <line x1="0" y1="0" x2="100" y2="100"
                    stroke="#c33" data-origami-kind="mountain"/>
              <line x1="0" y1="50" x2="100" y2="50"
                    stroke="#111" data-origami-kind="cut"/>
            </svg>"##;
    let preview = read_svg_preview(bytes).expect("read cut SVG preview");
    let rectangle = preview
        .boundary_candidates()
        .iter()
        .find(|candidate| candidate.kind == SvgBoundaryCandidateKind::Rectangle)
        .expect("rectangle boundary candidate");
    let mappings = preview
        .style_groups()
        .iter()
        .map(|group| SvgGroupMapping {
            group: group.id,
            target: match group.semantic.as_deref() {
                Some("mountain") => SvgGroupTarget::Mountain,
                Some("cut") => SvgGroupTarget::Cut,
                _ => SvgGroupTarget::Ignore,
            },
        })
        .collect::<Vec<_>>();

    let error = build_svg_import_replacement(
        bytes,
        SvgImportReplacementOptions {
            name: "切断確認".to_owned(),
            millimeters_per_unit: 1.0,
            group_mappings: mappings.clone(),
            boundary_candidate: Some(rectangle.id),
            boundary_confirmed: true,
            warnings_acknowledged: true,
            cutting_allowed_confirmed: false,
        },
    )
    .err()
    .expect("cutting must require explicit confirmation");
    assert!(error.contains("cutting must be explicitly allowed"));

    let replacement = build_svg_import_replacement(
        bytes,
        SvgImportReplacementOptions {
            name: "切断確認".to_owned(),
            millimeters_per_unit: 1.0,
            group_mappings: mappings,
            boundary_candidate: Some(rectangle.id),
            boundary_confirmed: true,
            warnings_acknowledged: true,
            cutting_allowed_confirmed: true,
        },
    )
    .expect("confirmed cut SVG must convert");
    let edges = &replacement.editor.pattern().edges;
    assert!(replacement.editor.paper().cutting_allowed);
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Mountain)
            .count(),
        2,
        "the mountain line must split at the X intersection"
    );
    assert_eq!(
        edges
            .iter()
            .filter(|edge| edge.kind == EdgeKind::Cut)
            .count(),
        2,
        "the cut line must split at the X intersection"
    );
    assert!(
        replacement.editor.paper().boundary_vertices.len() > 4,
        "cut contacts must split the paper boundary at both T junctions"
    );
}

fn solver_stage_fixture() -> (
    ProjectState,
    GeometricConstraintSolveStage,
    VertexId,
    Point2,
) {
    let start = VertexId::new();
    let end = VertexId::new();
    let original = Point2::new(0.0, 0.0);
    let mut project = ProjectState::new(CreasePattern {
        vertices: vec![
            ori_domain::Vertex {
                id: start,
                position: original,
            },
            ori_domain::Vertex {
                id: end,
                position: Point2::new(5.0, 0.0),
            },
        ],
        edges: vec![ori_domain::Edge {
            id: EdgeId::new(),
            start,
            end,
            kind: EdgeKind::Auxiliary,
        }],
    });
    project.saved_revision = Some(0);
    let stage = GeometricConstraintSolveStage {
        token: ProjectId::new(),
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: 0,
        positions: vec![(start, Point2::new(2.0, 3.0))],
        expression_bindings: None,
        exact_satisfaction: None,
    };
    (project, stage, start, original)
}

fn solver_vertex_position(project: &ProjectState, id: VertexId) -> Point2 {
    project
        .editor
        .pattern()
        .vertices
        .iter()
        .find(|vertex| vertex.id == id)
        .unwrap()
        .position
}

#[test]
fn constraint_solver_stale_token_is_atomic() {
    let (mut project, stage, vertex, original) = solver_stage_fixture();
    assert!(
        apply_geometric_constraint_solve_stage(
            &mut project,
            &stage,
            stage.project_instance_id,
            stage.project_id,
            0,
            ProjectId::new(),
        )
        .is_err()
    );
    assert_eq!(project.editor.revision(), 0);
    assert_eq!(solver_vertex_position(&project, vertex), original);
}

#[test]
fn constraint_solver_layer_lock_is_atomic() {
    let (mut project, mut stage, vertex, original) = solver_stage_fixture();
    let layer = project.editor.project_layers().layers[0].id;
    execute_command(
        &mut project,
        stage.project_id,
        0,
        Command::UpdateLayerPresentation {
            layer,
            visible: true,
            locked: true,
            opacity: 1.0,
        },
    )
    .unwrap();
    stage.revision = 1;
    assert!(
        apply_geometric_constraint_solve_stage(
            &mut project,
            &stage,
            stage.project_instance_id,
            stage.project_id,
            1,
            stage.token,
        )
        .is_err()
    );
    assert_eq!(project.editor.revision(), 1);
    assert_eq!(solver_vertex_position(&project, vertex), original);
}

#[test]
fn constraint_solver_apply_is_one_history_entry() {
    let (mut project, stage, _, _) = solver_stage_fixture();
    let snapshot = apply_geometric_constraint_solve_stage(
        &mut project,
        &stage,
        stage.project_instance_id,
        stage.project_id,
        0,
        stage.token,
    )
    .unwrap();
    assert_eq!(snapshot.revision, 1);
    assert!(snapshot.can_undo);
    assert!(!snapshot.can_redo);
}

#[test]
fn constraint_solver_undo_redo_restores_exact_positions() {
    let (mut project, stage, vertex, original) = solver_stage_fixture();
    let target = stage.positions[0].1;
    apply_geometric_constraint_solve_stage(
        &mut project,
        &stage,
        stage.project_instance_id,
        stage.project_id,
        0,
        stage.token,
    )
    .unwrap();
    execute_undo(&mut project, stage.project_id, 1).unwrap();
    assert_eq!(solver_vertex_position(&project, vertex), original);
    execute_redo(&mut project, stage.project_id, 2).unwrap();
    assert_eq!(solver_vertex_position(&project, vertex), target);
}

#[test]
fn saved_vertex_expressions_are_recomputed_as_multi_drivers() {
    let (mut project, _, vertex, _) = solver_stage_fixture();
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        vertex, "1+2", "sqrt(16)", 0.0, 0.0,
    )];
    assert_eq!(
        reevaluate_saved_vertex_expressions(&project).unwrap(),
        vec![(vertex, Point2::new(3.0, 4.0))]
    );
}

#[test]
fn saved_expression_duplicates_and_nonfinite_results_fail_closed() {
    let (mut project, _, vertex, _) = solver_stage_fixture();
    let valid = VertexCoordinateExpressions::new(vertex, "1", "2", 0.0, 0.0);
    project.numeric_expressions.vertex_coordinates = vec![valid.clone(), valid];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        vertex, "1/0", "2", 0.0, 0.0,
    )];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
}

#[test]
fn saved_expression_dependency_names_and_shared_vertex_cycles_fail_closed() {
    let (mut project, _, vertex, _) = solver_stage_fixture();
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        vertex,
        "vertex_x+1",
        "2",
        0.0,
        0.0,
    )];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
    let binding = VertexCoordinateExpressions::new(vertex, "1", "2", 0.0, 0.0);
    project.numeric_expressions.vertex_coordinates = vec![binding.clone(), binding];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
}

fn vertex_reference(id: VertexId, axis: char) -> String {
    let id = serde_json::to_value(id)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    format!("v.{id}.{axis}")
}

fn edge_reference(id: EdgeId, field: &str) -> String {
    let id = serde_json::to_value(id)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();
    format!("e.{id}.{field}")
}

static VERTEX_REFERENCE_TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn saved_vertex_reference_dag_is_evaluated_topologically() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, first, _) = solver_stage_fixture();
    let second = project.editor.pattern().vertices[1].id;
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(first, "2", "3", 0.0, 0.0),
        VertexCoordinateExpressions::new(
            second,
            format!("{}+4", vertex_reference(first, 'x')),
            format!("{}*2", vertex_reference(first, 'y')),
            0.0,
            0.0,
        ),
    ];
    assert_eq!(
        reevaluate_saved_vertex_expressions(&project).unwrap(),
        vec![
            (first, Point2::new(2.0, 3.0)),
            (second, Point2::new(6.0, 6.0)),
        ]
    );
}

#[test]
fn saved_vertex_reference_self_cycle_and_dangling_fail_closed() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, vertex, _) = solver_stage_fixture();
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        vertex,
        vertex_reference(vertex, 'x'),
        "0",
        0.0,
        0.0,
    )];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        vertex,
        vertex_reference(VertexId::new(), 'x'),
        "0",
        0.0,
        0.0,
    )];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
}

#[test]
fn vertex_reference_requires_lowercase_canonical_uuid_and_allows_equal_values() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, first, _) = solver_stage_fixture();
    let second = project.editor.pattern().vertices[1].id;
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(first, "2", "2", 2.0, 2.0),
        VertexCoordinateExpressions::new(second, "2", "2", 2.0, 2.0),
    ];
    reevaluate_saved_vertex_expressions(&project).expect("distinct bindings may share values");
    project.numeric_expressions.vertex_coordinates[1].x_source =
        vertex_reference(first, 'x').to_uppercase();
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
}

fn dependency_chain(project: &mut ProjectState, count: usize) {
    let ids = (0..count).map(|_| VertexId::new()).collect::<Vec<_>>();
    project.numeric_expressions.vertex_coordinates = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let source = ids
                .get(index + 1)
                .map_or_else(|| "1".to_owned(), |next| vertex_reference(*next, 'x'));
            VertexCoordinateExpressions::new(*id, source, "0", 0.0, 0.0)
        })
        .collect();
}

#[test]
fn vertex_reference_depth_64_is_allowed_and_65_is_rejected() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, _, _) = solver_stage_fixture();
    dependency_chain(&mut project, 65);
    assert!(reevaluate_saved_vertex_expressions(&project).is_ok());
    dependency_chain(&mut project, 66);
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
}

#[test]
fn vertex_reference_4096_boundary_is_bounded_and_4097_is_rejected() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (project, _, vertex, _) = solver_stage_fixture();
    let reference = vertex_reference(vertex, 'x');
    let source = std::iter::repeat_n(reference.as_str(), 4_096)
        .collect::<Vec<_>>()
        .join("+");
    let mut memo = HashMap::new();
    let mut visiting = HashSet::new();
    let mut work = 0;
    let started = std::time::Instant::now();
    assert!(
        expand_saved_vertex_references(&project, &source, &mut memo, &mut visiting, &mut work, 0,)
            .is_ok()
    );
    assert_eq!(work, 4_096);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "the maximum-size reference graph must remain bounded on loaded CI hosts"
    );
    let too_many = format!("{source}+{reference}");
    assert!(
        expand_saved_vertex_references(
            &project,
            &too_many,
            &mut HashMap::new(),
            &mut HashSet::new(),
            &mut 0,
            0,
        )
        .is_err()
    );
}

#[test]
fn referenced_expression_still_obeys_numeric_operation_limit() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, first, _) = solver_stage_fixture();
    let second = project.editor.pattern().vertices[1].id;
    let oversized = std::iter::repeat_n("1", 20_000)
        .collect::<Vec<_>>()
        .join("+");
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(first, oversized, "0", 0.0, 0.0),
        VertexCoordinateExpressions::new(second, vertex_reference(first, 'x'), "0", 0.0, 0.0),
    ];
    let started = std::time::Instant::now();
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

#[test]
fn saved_edge_length_and_angle_follow_endpoint_dag() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (project, _, start, _) = solver_stage_fixture();
    let edge = project.editor.pattern().edges[0].clone();
    let derived = VertexId::new();
    let mut pattern = project.editor.pattern().clone();
    pattern.vertices.push(ori_domain::Vertex {
        id: derived,
        position: Point2::new(0.0, 0.0),
    });
    let mut project = ProjectState::new(pattern);
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(start, "0", "0", 0.0, 0.0),
        VertexCoordinateExpressions::new(edge.end, "3", "4", 3.0, 4.0),
        VertexCoordinateExpressions::new(
            derived,
            edge_reference(edge.id, "length"),
            edge_reference(edge.id, "angle"),
            5.0,
            53.13010235415598,
        ),
    ];
    let values = reevaluate_saved_vertex_expressions(&project).unwrap();
    let point = values
        .iter()
        .find(|(vertex, _)| *vertex == derived)
        .unwrap()
        .1;
    assert_eq!(point.x, 5.0);
    assert!((point.y - 53.13010235415598).abs() <= 1e-12);
}

#[test]
fn saved_edge_reference_cycle_and_dangling_fail_closed() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, _, _) = solver_stage_fixture();
    let edge = project.editor.pattern().edges[0].clone();
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        edge.end,
        edge_reference(edge.id, "length"),
        "0",
        0.0,
        0.0,
    )];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        edge.end,
        edge_reference(EdgeId::new(), "length"),
        "0",
        0.0,
        0.0,
    )];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
}

#[test]
fn edge_angle_reversal_and_zero_boundary_are_canonical() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (project, _, start, _) = solver_stage_fixture();
    let original = project.editor.pattern().edges[0].clone();
    let reverse = EdgeId::new();
    let derived = VertexId::new();
    let mut pattern = project.editor.pattern().clone();
    pattern.edges.push(ori_domain::Edge {
        id: reverse,
        start: original.end,
        end: original.start,
        kind: EdgeKind::Auxiliary,
    });
    pattern.vertices.push(ori_domain::Vertex {
        id: derived,
        position: Point2::new(0.0, 0.0),
    });
    let mut project = ProjectState::new(pattern);
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(start, "0", "0", 0.0, 0.0),
        VertexCoordinateExpressions::new(original.end, "5", "0", 5.0, 0.0),
        VertexCoordinateExpressions::new(
            derived,
            edge_reference(original.id, "angle"),
            edge_reference(reverse, "angle"),
            0.0,
            180.0,
        ),
    ];
    let values = reevaluate_saved_vertex_expressions(&project).unwrap();
    let angle = values.iter().find(|(id, _)| *id == derived).unwrap().1;
    assert_eq!(angle, Point2::new(0.0, 180.0));
}

#[test]
fn zero_length_edge_reference_fails_closed() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (project, _, start, _) = solver_stage_fixture();
    let edge = project.editor.pattern().edges[0].clone();
    let derived = VertexId::new();
    let mut pattern = project.editor.pattern().clone();
    pattern.vertices.push(ori_domain::Vertex {
        id: derived,
        position: Point2::new(0.0, 0.0),
    });
    let mut project = ProjectState::new(pattern);
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(start, "1", "1", 0.0, 0.0),
        VertexCoordinateExpressions::new(edge.end, "1", "1", 0.0, 0.0),
        VertexCoordinateExpressions::new(derived, edge_reference(edge.id, "length"), "0", 0.0, 0.0),
    ];
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
}

#[test]
fn shared_edge_chain_is_memoized_and_indirect_cycle_is_rejected() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (project, _, first, _) = solver_stage_fixture();
    let first_edge = project.editor.pattern().edges[0].clone();
    let third = VertexId::new();
    let second_edge = EdgeId::new();
    let mut pattern = project.editor.pattern().clone();
    pattern.vertices.push(ori_domain::Vertex {
        id: third,
        position: Point2::new(0.0, 0.0),
    });
    pattern.edges.push(ori_domain::Edge {
        id: second_edge,
        start: first_edge.end,
        end: third,
        kind: EdgeKind::Auxiliary,
    });
    let mut project = ProjectState::new(pattern);
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(first, "0", "0", 0.0, 0.0),
        VertexCoordinateExpressions::new(first_edge.end, "3", "0", 3.0, 0.0),
        VertexCoordinateExpressions::new(
            third,
            format!("{}+4", edge_reference(first_edge.id, "length")),
            "0",
            7.0,
            0.0,
        ),
    ];
    assert!(reevaluate_saved_vertex_expressions(&project).is_ok());
    project.numeric_expressions.vertex_coordinates[1].x_source =
        edge_reference(second_edge, "length");
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
}

#[test]
fn referenced_expression_round_trip_detects_saved_value_tampering() {
    let _serial = VERTEX_REFERENCE_TEST_LOCK.lock().unwrap();
    let (mut project, _, first, _) = solver_stage_fixture();
    let second = project.editor.pattern().vertices[1].id;
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(first, "2", "3", 2.0, 3.0),
        VertexCoordinateExpressions::new(
            second,
            vertex_reference(first, 'x'),
            vertex_reference(first, 'y'),
            2.0,
            3.0,
        ),
    ];
    let mut document = project.document();
    for binding in &document.numeric_expressions.vertex_coordinates {
        let vertex = document
            .crease_pattern
            .vertices
            .iter_mut()
            .find(|vertex| vertex.id == binding.vertex)
            .unwrap();
        vertex.position = Point2::new(binding.adopted_x_mm, binding.adopted_y_mm);
    }
    assert!(validate_loaded_numeric_expression_bindings(&document).is_ok());
    document.numeric_expressions.vertex_coordinates[1].adopted_x_mm = 9.0;
    assert!(validate_loaded_numeric_expression_bindings(&document).is_err());
}

#[test]
fn ten_thousand_saved_expressions_are_rejected_before_evaluation_within_bound() {
    let (mut project, _, _, _) = solver_stage_fixture();
    project.numeric_expressions.vertex_coordinates = (0..10_000)
        .map(|_| VertexCoordinateExpressions::new(VertexId::new(), "1", "2", 1.0, 2.0))
        .collect();
    let started = std::time::Instant::now();
    assert!(reevaluate_saved_vertex_expressions(&project).is_err());
    assert!(started.elapsed() < std::time::Duration::from_millis(100));
}

#[test]
fn expression_reexecution_after_undo_redo_uses_the_restored_binding() {
    let (mut project, mut stage, vertex, _) = solver_stage_fixture();
    let dependent = project.editor.pattern().vertices[1].id;
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(vertex, "2", "3", 0.0, 0.0),
        VertexCoordinateExpressions::new(
            dependent,
            format!("{}+1", vertex_reference(vertex, 'x')),
            format!("{}+1", vertex_reference(vertex, 'y')),
            0.0,
            0.0,
        ),
    ];
    stage.positions.push((dependent, Point2::new(3.0, 4.0)));
    stage.expression_bindings = Some(
        project
            .numeric_expressions
            .vertex_coordinates
            .iter()
            .cloned()
            .zip([Point2::new(2.0, 3.0), Point2::new(3.0, 4.0)])
            .map(|(mut binding, point)| {
                binding.adopted_x_mm = point.x;
                binding.adopted_y_mm = point.y;
                binding
            })
            .collect(),
    );
    apply_geometric_constraint_solve_stage(
        &mut project,
        &stage,
        stage.project_instance_id,
        stage.project_id,
        0,
        stage.token,
    )
    .unwrap();
    execute_undo(&mut project, stage.project_id, 1).unwrap();
    execute_redo(&mut project, stage.project_id, 2).unwrap();
    let mut actual = reevaluate_saved_vertex_expressions(&project).unwrap();
    actual.sort_unstable_by_key(|(vertex, _)| vertex.canonical_bytes());
    let mut expected = vec![
        (vertex, Point2::new(2.0, 3.0)),
        (dependent, Point2::new(3.0, 4.0)),
    ];
    expected.sort_unstable_by_key(|(vertex, _)| vertex.canonical_bytes());
    assert_eq!(actual, expected);
}

#[test]
fn expression_reexecution_survives_project_document_round_trip() {
    let (mut project, _, vertex, _) = solver_stage_fixture();
    project.numeric_expressions.vertex_coordinates = vec![VertexCoordinateExpressions::new(
        vertex, "6/2", "sqrt(16)", 3.0, 4.0,
    )];
    let reopened = ProjectState::from_valid_document(
        project.document(),
        PathBuf::from("expression-round-trip.ori2"),
    );
    assert_eq!(
        reevaluate_saved_vertex_expressions(&reopened).unwrap(),
        vec![(vertex, Point2::new(3.0, 4.0))]
    );
}

#[test]
fn saved_expression_constraint_conflict_does_not_mutate_project() {
    let (mut project, _, start, original) = solver_stage_fixture();
    let edge = project.editor.pattern().edges[0].clone();
    let project_id = project.project_id;
    execute_command(
        &mut project,
        project_id,
        0,
        Command::AddGeometricConstraint {
            record: GeometricConstraintRecordV1 {
                id: ConstraintId::new(),
                constraint: GeometricConstraintKindV1::FixedLength {
                    edge: edge.id,
                    length_mm: 1.0,
                },
            },
        },
    )
    .unwrap();
    project.numeric_expressions.vertex_coordinates = vec![
        VertexCoordinateExpressions::new(start, "0", "0", 0.0, 0.0),
        VertexCoordinateExpressions::new(edge.end, "2", "0", 2.0, 0.0),
    ];
    let drivers = reevaluate_saved_vertex_expressions(&project).unwrap();
    assert!(
        solve_geometric_constraints_with_drivers_v1(
            project.editor.pattern(),
            project.editor.geometric_constraints(),
            &drivers,
            ConstraintSolveLimitsV1::default(),
        )
        .is_err()
    );
    assert_eq!(project.editor.revision(), 1);
    assert_eq!(solver_vertex_position(&project, start), original);
}

#[test]
fn folded_landmark_ranking_rejects_collision_and_resource_one_over_without_mutation() {
    let reference = BeginnerReferenceModelSuggestionV1 {
        asset_id: AssetId::new(),
        bbox_min_tenths_mm: [0, 0, 0],
        bbox_max_tenths_mm: [100, 80, 40],
        dominant_normal_milli: [0, 0, 1000],
        surface_area_milli: 8_000,
        surface_landmarks_tenths_mm: vec![[0, 0, 0], [100, 80, 40]],
        surface_ranges: Vec::new(),
        protrusions: Vec::new(),
        general_protrusion_candidates: Vec::new(),
        stick_bars: Vec::new(),
        component_count: 1,
        inferred_component_bridges: false,
        principal_axis_extents_tenths_mm: [100, 80, 40],
        quality_score: 0,
        quality_reasons: vec!["strict_glb_vertex_index_bounds".to_owned()],
        insufficiency_reasons: vec!["insufficient_distinct_vertices".to_owned()],
        pair_bindings: Vec::new(),
        method: "test".to_owned(),
        suggested_part_kind: None,
    };
    let plan = |vertices: Vec<Vertex>, edges: Vec<Edge>| ori_domain::BeginnerGeneratedPlanV1 {
        schema_version: 1,
        kind: ori_domain::BeginnerGeneratedPlanKindV1::SymmetricFourLegBase,
        crease_pattern: CreasePattern { vertices, edges },
        instruction_codes: Vec::new(),
        target_parts: Vec::new(),
        skeleton_segments: Vec::new(),
        target_asset: None,
        semantic_landmark_provenance: None,
    };
    let a = VertexId::new();
    let b = VertexId::new();
    let c = VertexId::new();
    let collision = plan(
        vec![
            Vertex {
                id: a,
                position: Point2::new(1.0, 1.0),
            },
            Vertex {
                id: b,
                position: Point2::new(1.0, 1.0),
            },
            Vertex {
                id: c,
                position: Point2::new(2.0, 1.0),
            },
        ],
        vec![Edge {
            id: EdgeId::new(),
            start: a,
            end: c,
            kind: EdgeKind::Mountain,
        }],
    );
    assert_eq!(
        bounded_folded_pose_landmark_score_v1(&collision, &reference),
        None
    );
    let tiny_end = VertexId::new();
    let tiny = CreasePattern {
        vertices: vec![
            Vertex {
                id: a,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: tiny_end,
                position: Point2::new(1.0e-12, 0.0),
            },
        ],
        edges: vec![Edge {
            id: EdgeId::new(),
            start: a,
            end: tiny_end,
            kind: EdgeKind::Valley,
        }],
    };
    assert_eq!(
        validate_beginner_manufacturability_v1(&tiny, &Paper::default()),
        Err("manufacturability_minimum_crease_spacing")
    );
    let p0 = VertexId::new();
    let p1 = VertexId::new();
    let p2 = VertexId::new();
    let _manufacturable_sequence = plan(
        vec![
            Vertex {
                id: p0,
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: p1,
                position: Point2::new(10.0, 0.0),
            },
            Vertex {
                id: p2,
                position: Point2::new(0.0, 10.0),
            },
        ],
        vec![
            Edge {
                id: EdgeId::new(),
                start: p0,
                end: p1,
                kind: EdgeKind::Mountain,
            },
            Edge {
                id: EdgeId::new(),
                start: p1,
                end: p2,
                kind: EdgeKind::Valley,
            },
        ],
    );
    let boundary_vertices = [
        (0.0, 0.0),
        (5.0, 0.0),
        (10.0, 0.0),
        (10.0, 10.0),
        (5.0, 10.0),
        (0.0, 10.0),
    ]
    .into_iter()
    .map(|(x, y)| Vertex {
        id: VertexId::new(),
        position: Point2::new(x, y),
    })
    .collect::<Vec<_>>();
    let boundary_ids = boundary_vertices
        .iter()
        .map(|vertex| vertex.id)
        .collect::<Vec<_>>();
    let certified_edge = Edge {
        id: EdgeId::new(),
        start: boundary_ids[1],
        end: boundary_ids[4],
        kind: EdgeKind::Mountain,
    };
    let certified = plan(
        vec![boundary_vertices[1].clone(), boundary_vertices[4].clone()],
        vec![certified_edge.clone()],
    );
    let mut certified_edges = (0..boundary_ids.len())
        .map(|index| Edge {
            id: EdgeId::new(),
            start: boundary_ids[index],
            end: boundary_ids[(index + 1) % boundary_ids.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    certified_edges.push(certified_edge);
    let certified_pattern = CreasePattern {
        vertices: boundary_vertices,
        edges: certified_edges,
    };
    let certified_paper = Paper {
        boundary_vertices: boundary_ids,
        thickness_mm: 0.0,
        ..Paper::default()
    };
    let certified_editor =
        EditorState::with_paper(certified_pattern.clone(), certified_paper.clone());
    let certified_topology = certified_editor
        .topology_analysis_input(ProjectId::new())
        .analyze();
    let certified_topology = certified_topology
        .simulation_snapshot()
        .expect("certificate fixture topology");
    assert_eq!(
        certify_beginner_fold_path_v1(
            &certified,
            &certified_paper,
            &certified_pattern,
            certified_topology,
        ),
        certify_beginner_fold_path_v1(
            &certified,
            &certified_paper,
            &certified_pattern,
            certified_topology,
        )
    );
    let ranked = plan(
        vec![
            Vertex {
                id: VertexId::new(),
                position: Point2::new(0.0, 0.0),
            },
            Vertex {
                id: VertexId::new(),
                position: Point2::new(10.0, 8.0),
            },
        ],
        Vec::new(),
    );
    let shape_profile = ori_domain::BeginnerDesignProfileV1 {
        preset: ori_domain::BeginnerDesignPresetV1::ShapePriority,
        shape_fidelity_weight: 60,
        foldability_weight: 20,
        step_count_weight: 10,
        paper_efficiency_weight: 10,
        ..ori_domain::BeginnerDesignProfileV1::default()
    };
    let fold_profile = ori_domain::BeginnerDesignProfileV1 {
        preset: ori_domain::BeginnerDesignPresetV1::FoldabilityPriority,
        shape_fidelity_weight: 20,
        foldability_weight: 60,
        step_count_weight: 10,
        paper_efficiency_weight: 10,
        ..ori_domain::BeginnerDesignProfileV1::default()
    };
    assert_ne!(
        preset_weighted_refinement_score_v1(&ranked, &reference, &shape_profile),
        preset_weighted_refinement_score_v1(&ranked, &reference, &fold_profile),
    );

    let oversized = plan(
        (0..=MAX_BEGINNER_FOLDED_LANDMARKS_V1)
            .map(|index| Vertex {
                id: VertexId::new(),
                position: Point2::new(index as f64, 0.0),
            })
            .collect(),
        Vec::new(),
    );
    assert_eq!(
        bounded_folded_pose_landmark_score_v1(&oversized, &reference),
        None
    );
    let project = initial_project_state();
    let before = project.document();
    let _ = bounded_folded_pose_landmark_score_v1(&oversized, &reference);
    assert_eq!(project.document(), before);
}

#[test]
fn linear_array_preview_and_confirm_are_bound_read_only_and_atomic() {
    let sheet = create_rectangular_sheet(100.0, 100.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let mut vertices = [VertexId::new(), VertexId::new()];
    vertices.sort_by_key(|id| id.canonical_bytes());
    let edge = EdgeId::new();
    pattern.vertices.extend([
        Vertex {
            id: vertices[0],
            position: Point2::new(20.0, 20.0),
        },
        Vertex {
            id: vertices[1],
            position: Point2::new(30.0, 20.0),
        },
    ]);
    pattern.edges.push(Edge {
        id: edge,
        start: vertices[0],
        end: vertices[1],
        kind: EdgeKind::Mountain,
    });
    let mut project = ProjectState::new_with_paper(pattern, paper);
    let instance = project.instance_id;
    let id = project.project_id;
    let request = LinearArrayRequestV1 {
        vertices: vertices.to_vec(),
        edges: vec![edge],
        additional_copies: 1,
        delta: Point2::new(0.0, 10.0),
    };
    let before = project.document();
    let preview = preview_linear_array_inner(&project, instance, id, 0, request.clone()).unwrap();
    assert_eq!(project.document(), before);
    assert!(!preview.authorizes_project_mutation);
    for (bad_instance, bad_id, bad_revision) in [
        (ProjectId::new(), id, 0),
        (instance, ProjectId::new(), 0),
        (instance, id, 1),
    ] {
        assert!(
            preview_linear_array_inner(
                &project,
                bad_instance,
                bad_id,
                bad_revision,
                request.clone()
            )
            .is_err()
        );
        assert_eq!(project.document(), before);
    }
    let mut bad_digest = preview.request_sha256.clone();
    bad_digest.replace_range(0..1, if &bad_digest[0..1] == "0" { "1" } else { "0" });
    assert!(
        confirm_linear_array_inner(&mut project, instance, id, 0, request.clone(), bad_digest)
            .is_err()
    );
    assert_eq!(project.document(), before);
    let mut changed_request = request.clone();
    changed_request.delta.y = 11.0;
    assert!(
        confirm_linear_array_inner(
            &mut project,
            instance,
            id,
            0,
            changed_request,
            preview.request_sha256.clone(),
        )
        .is_err()
    );
    assert_eq!(project.document(), before);
    let snapshot = confirm_linear_array_inner(
        &mut project,
        instance,
        id,
        0,
        request.clone(),
        preview.request_sha256,
    )
    .unwrap();
    assert_eq!(snapshot.revision, 1);
    assert!(
        confirm_linear_array_inner(&mut project, instance, id, 0, request, String::new()).is_err()
    );
    project.editor.undo(1).unwrap();
    assert_eq!(project.editor.pattern(), &before.crease_pattern);
    let restored = project.document();
    let invalid = LinearArrayRequestV1 {
        vertices: vertices.to_vec(),
        edges: vec![edge],
        additional_copies: 0,
        delta: Point2::new(0.0, 10.0),
    };
    assert!(preview_linear_array_inner(&project, instance, id, 2, invalid).is_err());
    assert_eq!(project.document(), restored);
}

#[test]
fn radial_array_preview_and_confirm_are_domain_bound_read_only_and_atomic() {
    let sheet = create_rectangular_sheet(100.0, 100.0, false).unwrap();
    let (mut pattern, paper) = sheet.into_parts();
    let center = VertexId::new();
    let outer = VertexId::new();
    let edge = EdgeId::new();
    pattern.vertices.extend([
        Vertex {
            id: center,
            position: Point2::new(50.0, 50.0),
        },
        Vertex {
            id: outer,
            position: Point2::new(60.0, 50.0),
        },
    ]);
    pattern.edges.push(Edge {
        id: edge,
        start: center,
        end: outer,
        kind: EdgeKind::Mountain,
    });
    let mut project = ProjectState::new_with_paper(pattern, paper);
    let instance = project.instance_id;
    let id = project.project_id;
    let mut vertices = vec![center, outer];
    vertices.sort_by_key(|id| id.canonical_bytes());
    let request = RadialArrayRequestV1 {
        center,
        vertices,
        edges: vec![edge],
        additional_copies: 1,
        angle_microdegrees: 90_000_000,
    };
    let before = project.document();
    let preview = preview_radial_array_inner(&project, instance, id, 0, request.clone()).unwrap();
    assert_eq!(project.document(), before);
    assert!(!preview.authorizes_project_mutation);
    assert!(
        preview_radial_array_inner(&project, ProjectId::new(), id, 0, request.clone()).is_err()
    );
    assert!(
        preview_radial_array_inner(&project, instance, ProjectId::new(), 0, request.clone())
            .is_err()
    );
    assert!(preview_radial_array_inner(&project, instance, id, 1, request.clone()).is_err());
    assert_eq!(project.document(), before);
    let linear = LinearArrayRequestV1 {
        vertices: request.vertices.clone(),
        edges: request.edges.clone(),
        additional_copies: 1,
        delta: Point2::new(0.0, 10.0),
    };
    assert_ne!(
        preview.request_sha256,
        linear_array_request_sha256(instance, id, 0, &linear).unwrap()
    );
    let mut changed = request.clone();
    changed.angle_microdegrees = 180_000_000;
    assert!(
        confirm_radial_array_inner(
            &mut project,
            instance,
            id,
            0,
            changed,
            preview.request_sha256.clone()
        )
        .is_err()
    );
    assert_eq!(project.document(), before);
    assert!(
        confirm_radial_array_inner(
            &mut project,
            instance,
            id,
            0,
            request.clone(),
            "0".repeat(64)
        )
        .is_err()
    );
    assert_eq!(project.document(), before);
    let snapshot = confirm_radial_array_inner(
        &mut project,
        instance,
        id,
        0,
        request,
        preview.request_sha256,
    )
    .unwrap();
    assert_eq!(snapshot.revision, 1);
    project.editor.undo(1).unwrap();
    assert_eq!(project.editor.pattern(), &before.crease_pattern);
}
