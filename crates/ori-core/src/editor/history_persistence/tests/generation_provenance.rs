use ori_domain::{
    AssetId, BeginnerBulgeTargetV1, BeginnerGenerationProvenanceV1,
    BeginnerReferenceSurfaceBindingV1, BeginnerTargetAssetReferenceV1, FaceId, LayerContentKindV1,
    MAX_UNDERLAYS_V1, UnderlayTransformV1,
};

use super::*;

fn underlay_record(id: UnderlayId, asset: AssetId, layer: LayerId) -> UnderlayRecordV1 {
    UnderlayRecordV1 {
        id,
        asset,
        transform: UnderlayTransformV1 {
            position: Point2::new(12.0, 24.0),
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_degrees: 0.0,
        },
        opacity: 0.75,
        layer,
    }
}

fn nil_underlay_id() -> UnderlayId {
    serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"")
        .expect("decode nil underlay ID fixture")
}

fn nil_asset_id() -> AssetId {
    serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"")
        .expect("decode nil asset ID fixture")
}

fn nil_layer_id() -> LayerId {
    serde_json::from_str("\"00000000-0000-0000-0000-000000000000\"")
        .expect("decode nil layer ID fixture")
}

fn beginner_profile_with_underlay_provenance(
    underlay_id: UnderlayId,
    asset_id: AssetId,
    topology_authority_byte: u8,
) -> BeginnerDesignProfileV1 {
    let mut profile = BeginnerDesignProfileV1::default();
    profile.generation_constraints.target_asset =
        Some(BeginnerTargetAssetReferenceV1::ReferenceImage {
            underlay_id,
            asset_id,
        });
    profile.generation_provenance = Some(BeginnerGenerationProvenanceV1 {
        schema_version: 1,
        topology_authority_sha256: [topology_authority_byte; 32],
        fold_path_certificate_sha256: Some([topology_authority_byte.wrapping_add(1); 32]),
        document_authority_sha256: None,
        confidence_score: 90,
        confidence_reasons: vec!["history_test_authority_v1".to_owned()],
        explicit_override: false,
        source_asset_fingerprint: format!("history-test-source-{topology_authority_byte:02x}"),
        semantic_landmark_provenance: None,
        generic_tree: None,
        reference_consensus: None,
        reference_consensus_summary: None,
    });
    profile
}

struct UnderlayProvenanceHistoryFixture {
    editor: EditorState,
    target_before: UnderlayRecordV1,
    target_after: UnderlayRecordV1,
}

fn underlay_provenance_history_fixture() -> UnderlayProvenanceHistoryFixture {
    let sheet = crate::create_rectangular_sheet(100.0, 60.0, false).expect("valid history fixture");
    let (pattern, paper) = sheet.into_parts();
    let underlay_layer = LayerRecordV1 {
        id: LayerId::new(),
        name: "Reference authority".to_owned(),
        content_kind: LayerContentKindV1::Underlay,
        visible: true,
        locked: false,
        opacity: 1.0,
    };
    let mut project_layers = ProjectLayerDocumentV1::default();
    project_layers.layers.push(underlay_layer.clone());
    let target_before = underlay_record(UnderlayId::new(), AssetId::new(), underlay_layer.id);
    let underlays = UnderlayDocumentV1 {
        schema_version: ori_domain::UNDERLAY_SCHEMA_VERSION_V1,
        underlays: vec![target_before.clone()],
    };
    let mut editor = EditorState::with_all_document_parts_annotations_underlays_and_memo(
        pattern,
        paper,
        InstructionTimeline::default(),
        GeometricConstraintDocumentV1::default(),
        project_layers,
        ElementMetadataDocumentV1::default(),
        AnnotationDocumentV1::default(),
        underlays,
        String::new(),
    );
    let proven_profile =
        beginner_profile_with_underlay_provenance(target_before.id, target_before.asset, 0x31);
    let mut generated_timeline = editor.instruction_timeline().clone();
    generated_timeline
        .steps
        .push(declarative_instruction_step("Generated authority"));
    editor
        .execute(
            editor.revision(),
            Command::ApplyBeginnerGeneratedDocument {
                pattern: editor.pattern().clone(),
                paper: editor.paper().clone(),
                instruction_timeline: generated_timeline,
                project_layers: editor.project_layers().clone(),
                beginner_design_profile: Box::new(proven_profile.clone()),
            },
        )
        .expect("mint generation provenance through the generated-document command");
    let mut target_after = target_before.clone();
    target_after.asset = AssetId::new();
    editor
        .execute(
            editor.revision(),
            Command::UpdateUnderlay {
                record: target_after.clone(),
            },
        )
        .expect("invalidate provenance through the target asset");
    assert!(
        editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );

    UnderlayProvenanceHistoryFixture {
        editor,
        target_before,
        target_after,
    }
}

#[test]
fn current_generation_provenance_requires_its_applied_authority_edge() {
    let fixture = underlay_provenance_history_fixture();
    let mut editor = fixture.editor;
    editor
        .undo(editor.revision())
        .expect("restore the generated positive provenance");
    let current_profile = editor.beginner_design_profile().clone();
    let history = editor
        .export_history_v1(ProjectId::new())
        .expect("export applied generation history");
    assert!(
        history.authenticates_current_beginner_generation_provenance_v1(&current_profile),
        "the applied beginner-generation edge independently authenticates its current claim"
    );

    let mut redo_only = history.clone();
    let authority_edge = redo_only
        .undo_stack
        .pop()
        .expect("fixture contains its applied authority edge");
    redo_only.redo_stack.push(authority_edge);
    assert!(
        !redo_only.authenticates_current_beginner_generation_provenance_v1(&current_profile),
        "an unapplied Redo edge cannot authenticate the current document"
    );

    editor
        .execute(
            editor.revision(),
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(current_profile.clone()),
            },
        )
        .expect("an exact profile no-op preserves provenance");
    let history_with_noop = editor
        .export_history_v1(ProjectId::new())
        .expect("export authority-preserving history");
    assert!(
        history_with_noop.authenticates_current_beginner_generation_provenance_v1(&current_profile),
        "an exact later profile no-op must not hide the earlier authority edge"
    );

    let mut unrelated = EditorState::new(CreasePattern::empty());
    unrelated
        .execute(
            unrelated.revision(),
            Command::UpdateProjectMemo {
                memo: "unrelated history".to_owned(),
            },
        )
        .expect("create unrelated non-empty history");
    let unrelated_history = unrelated
        .export_history_v1(ProjectId::new())
        .expect("export unrelated history");
    assert!(
        !unrelated_history
            .authenticates_current_beginner_generation_provenance_v1(&current_profile),
        "non-empty unrelated history must not substitute for generation authority"
    );
}

fn combined_underlay_profile_inverse_mut(
    history: &mut EditorHistoryV1,
) -> (&mut UnderlayDocumentV1, &mut BeginnerDesignProfileV1) {
    let mut profile = history
        .undo_stack
        .iter()
        .find_map(|entry| match &entry.forward {
            CommandV1::ApplyBeginnerGeneratedDocument {
                beginner_design_profile,
                ..
            } => Some(beginner_design_profile.as_ref().clone()),
            _ => None,
        })
        .expect("fixture must persist the generated profile transition");
    let entry = history
        .undo_stack
        .iter_mut()
        .rev()
        .find(|entry| {
            matches!(
                &entry.inverse,
                InverseV1::RestoreBeginnerGenerationProvenance { inner, .. }
                    if matches!(
                        inner.as_ref(),
                        InverseV1::Command {
                            command: CommandV1::UpdateUnderlay { .. }
                        }
                    )
            )
        })
        .expect("fixture must persist the bounded provenance inverse");
    let (underlays, profile) = match &entry.inverse {
        InverseV1::RestoreBeginnerGenerationProvenance {
            provenance, inner, ..
        } => {
            let InverseV1::Command {
                command: CommandV1::UpdateUnderlay { record },
            } = inner.as_ref()
            else {
                unreachable!("fixture lookup admitted only an underlay update inverse")
            };
            profile.generation_provenance = Some(
                provenance
                    .as_ref()
                    .expect("new compact wrapper carries provenance")
                    .as_ref()
                    .clone(),
            );
            (
                UnderlayDocumentV1 {
                    schema_version: ori_domain::UNDERLAY_SCHEMA_VERSION_V1,
                    underlays: vec![record.clone()],
                },
                Box::new(profile),
            )
        }
        _ => unreachable!("fixture lookup admitted only a provenance wrapper"),
    };
    entry.inverse = InverseV1::RestoreUnderlaysAndBeginnerDesignProfile { underlays, profile };
    match &mut entry.inverse {
        InverseV1::RestoreUnderlaysAndBeginnerDesignProfile { underlays, profile } => {
            (underlays, profile.as_mut())
        }
        _ => unreachable!("fixture inverse was replaced with its legacy representation"),
    }
}

fn bounded_underlay_provenance_inverse_mut(
    history: &mut EditorHistoryV1,
) -> (
    &mut Option<[u8; 32]>,
    &mut Option<Box<BeginnerGenerationProvenanceV1>>,
    &mut UnderlayRecordV1,
) {
    history
        .undo_stack
        .iter_mut()
        .rev()
        .find_map(|entry| match &mut entry.inverse {
            InverseV1::RestoreBeginnerGenerationProvenance {
                profile_authority_sha256,
                provenance,
                inner,
                ..
            } => match inner.as_mut() {
                InverseV1::Command {
                    command: CommandV1::UpdateUnderlay { record },
                } => Some((profile_authority_sha256, provenance, record)),
                _ => None,
            },
            _ => None,
        })
        .expect("fixture must persist a bounded underlay provenance inverse")
}

fn restore_all(
    editor: &EditorState,
    history: EditorHistoryV1,
) -> Result<EditorState, EditorHistoryErrorV1> {
    EditorState::with_all_document_parts_annotations_underlays_memo_profile_and_history_v1(
        editor.pattern().clone(),
        editor.paper().clone(),
        editor.instruction_timeline().clone(),
        editor.geometric_constraints().clone(),
        editor.project_layers().clone(),
        editor.element_metadata().clone(),
        editor.annotations().clone(),
        editor.underlays().clone(),
        editor.project_memo().to_owned(),
        editor.beginner_design_profile().clone(),
        history,
    )
}

#[test]
fn generation_provenance_inverse_wrapper_rejects_nested_runtime_and_wire_values() {
    let vertex = VertexId::new();
    let profile =
        beginner_profile_with_underlay_provenance(UnderlayId::new(), AssetId::new(), 0x21);
    let profile_authority_sha256 = beginner_design_profile_authority_sha256_v1(&profile);
    let provenance = profile
        .generation_provenance
        .clone()
        .expect("fixture provenance");
    let runtime = Inverse::RestoreBeginnerGenerationProvenance {
        profile_authority_sha256,
        provenance: Box::new(provenance.clone()),
        inner: Box::new(Inverse::RestoreBeginnerGenerationProvenance {
            profile_authority_sha256,
            provenance: Box::new(provenance.clone()),
            inner: Box::new(Inverse::Command(Command::RemoveVertex { id: vertex })),
        }),
    };
    assert_eq!(
        inverse_to_wire(&runtime),
        Err(EditorHistoryErrorV1::InvalidInverse)
    );

    let wire = InverseV1::RestoreBeginnerGenerationProvenance {
        profile: None,
        profile_authority_sha256: Some(profile_authority_sha256),
        provenance: Some(Box::new(provenance.clone())),
        inner: Box::new(InverseV1::RestoreBeginnerGenerationProvenance {
            profile: None,
            profile_authority_sha256: Some(profile_authority_sha256),
            provenance: Some(Box::new(provenance)),
            inner: Box::new(InverseV1::Command {
                command: CommandV1::RemoveVertex { id: vertex },
            }),
        }),
    };
    assert_eq!(
        inverse_from_wire(wire),
        Err(EditorHistoryErrorV1::InvalidInverse)
    );
}

fn provenance_invalidating_move_history(vertex_count: usize) -> EditorHistoryV1 {
    assert!(vertex_count > 0);
    let mut pattern = CreasePattern::empty();
    pattern.vertices = (0..vertex_count)
        .map(|index| Vertex {
            id: VertexId::new(),
            position: Point2::new(index as f64 * 0.25, index as f64 * 0.5),
        })
        .collect();
    let moved = pattern.vertices[0].clone();
    let mut editor = EditorState::new(pattern);
    let profile = BeginnerDesignProfileV1 {
        generation_provenance: Some(BeginnerGenerationProvenanceV1 {
            schema_version: 1,
            topology_authority_sha256: [0x71; 32],
            fold_path_certificate_sha256: Some([0x72; 32]),
            document_authority_sha256: None,
            confidence_score: 90,
            confidence_reasons: vec!["bounded_history_regression_v1".to_owned()],
            explicit_override: false,
            source_asset_fingerprint: "bounded-history-regression-source-v1".to_owned(),
            semantic_landmark_provenance: None,
            generic_tree: None,
            reference_consensus: None,
            reference_consensus_summary: None,
        }),
        ..Default::default()
    };
    assert!(validate_beginner_design_profile_v1(&profile));
    editor
        .restore_beginner_design_profile(profile)
        .expect("restore valid generated profile");
    editor
        .execute(
            0,
            Command::MoveVertex {
                id: moved.id,
                position: Point2::new(moved.position.x + 0.125, moved.position.y + 0.25),
            },
        )
        .expect("move one vertex");
    let history = editor
        .export_history_v1(ProjectId::new())
        .expect("export provenance-invalidating move history");
    assert!(matches!(
        &history.undo_stack[0].inverse,
        InverseV1::RestoreBeginnerGenerationProvenance { inner, .. }
            if matches!(
                inner.as_ref(),
                InverseV1::Command {
                    command: CommandV1::MoveVertex { .. }
                }
            )
    ));
    history
}

#[test]
fn generation_provenance_history_bytes_do_not_scale_with_the_fold_document() {
    let small = provenance_invalidating_move_history(1);
    let large = provenance_invalidating_move_history(4_096);
    let small_bytes = serde_json::to_vec(&small)
        .expect("encode small history")
        .len();
    let large_bytes = serde_json::to_vec(&large)
        .expect("encode large history")
        .len();

    assert!(
        large_bytes <= small_bytes + 256,
        "one-vertex move history grew with the document: small={small_bytes}, large={large_bytes}"
    );
}

#[test]
fn unanchored_persisted_positive_provenance_is_rejected_on_reopen() {
    let history = provenance_invalidating_move_history(1);
    let mut pattern = CreasePattern::empty();
    let moved = match &history.undo_stack[0].forward {
        CommandV1::MoveVertex { id, position } => Vertex {
            id: *id,
            position: *position,
        },
        _ => panic!("fixture must contain a move"),
    };
    pattern.vertices.push(moved);
    let editor = EditorState::new(pattern);
    assert_eq!(
        restore_all(&editor, history)
            .expect_err("history alone must not manufacture positive provenance"),
        EditorHistoryErrorV1::InverseMismatch
    );
}

#[test]
fn bounded_generation_provenance_inverse_rejects_invalid_restored_authority_binding() {
    let fixture = underlay_provenance_history_fixture();
    let history = fixture
        .editor
        .export_history_v1(ProjectId::new())
        .expect("export authority-invalidating history");

    let mut missing_provenance = history.clone();
    *bounded_underlay_provenance_inverse_mut(&mut missing_provenance).1 = None;
    assert_eq!(
        restore_all(&fixture.editor, missing_provenance)
            .expect_err("bounded inverse without provenance must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut mismatched_profile_authority = history.clone();
    bounded_underlay_provenance_inverse_mut(&mut mismatched_profile_authority)
        .0
        .as_mut()
        .expect("compact wrapper authority digest")[0] ^= 0xff;
    assert_eq!(
        restore_all(&fixture.editor, mismatched_profile_authority)
            .expect_err("profile authority digest must bind the separately persisted profile"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut mismatched_inner_asset = history;
    bounded_underlay_provenance_inverse_mut(&mut mismatched_inner_asset)
        .2
        .asset = AssetId::new();
    assert_eq!(
        restore_all(&fixture.editor, mismatched_inner_asset)
            .expect_err("inner inverse must restore the profile's exact target asset"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut tampered_provenance = fixture
        .editor
        .export_history_v1(ProjectId::new())
        .expect("export authority-invalidating history");
    bounded_underlay_provenance_inverse_mut(&mut tampered_provenance)
        .1
        .as_mut()
        .expect("compact wrapper provenance")
        .topology_authority_sha256[0] ^= 0xff;
    assert_eq!(
        restore_all(&fixture.editor, tampered_provenance)
            .expect_err("forward replay must reject tampered compact provenance"),
        EditorHistoryErrorV1::InverseMismatch
    );
}

#[test]
fn legacy_full_profile_provenance_wrapper_reads_into_compact_runtime_form() {
    let profile =
        beginner_profile_with_underlay_provenance(UnderlayId::new(), AssetId::new(), 0x42);
    let expected_authority = beginner_design_profile_authority_sha256_v1(&profile);
    let expected_provenance = profile
        .generation_provenance
        .clone()
        .expect("legacy profile provenance");
    let runtime = inverse_from_wire(InverseV1::RestoreBeginnerGenerationProvenance {
        profile: Some(Box::new(profile)),
        profile_authority_sha256: None,
        provenance: None,
        inner: Box::new(InverseV1::Command {
            command: CommandV1::UpdateProjectMemo {
                memo: "before".to_owned(),
            },
        }),
    })
    .expect("read legacy full-profile wrapper");
    let InverseV1::RestoreBeginnerGenerationProvenance {
        profile,
        profile_authority_sha256,
        provenance,
        ..
    } = inverse_to_wire(&runtime).expect("rewrite compact wrapper")
    else {
        panic!("rewritten wrapper must retain its wire tag")
    };
    assert!(profile.is_none());
    assert_eq!(profile_authority_sha256, Some(expected_authority));
    assert_eq!(
        provenance.as_deref(),
        Some(&expected_provenance),
        "rewritten wrapper must retain only bounded evidence"
    );
    let Inverse::RestoreBeginnerGenerationProvenance {
        profile_authority_sha256,
        provenance,
        ..
    } = runtime
    else {
        panic!("legacy wrapper must become compact runtime form")
    };
    assert_eq!(profile_authority_sha256, expected_authority);
    assert_eq!(*provenance, expected_provenance);
}

#[test]
fn reopened_legacy_full_profile_wrapper_is_resaved_compactly() {
    let fixture = underlay_provenance_history_fixture();
    let project_id = ProjectId::new();
    let mut history = fixture
        .editor
        .export_history_v1(project_id)
        .expect("export compact fixture");
    let legacy_profile = history
        .undo_stack
        .iter()
        .find_map(|entry| match &entry.forward {
            CommandV1::ApplyBeginnerGeneratedDocument {
                beginner_design_profile,
                ..
            } => Some(beginner_design_profile.clone()),
            _ => None,
        })
        .expect("fixture generated profile");
    let wrapper = history
        .undo_stack
        .iter_mut()
        .find_map(|entry| match &mut entry.inverse {
            InverseV1::RestoreBeginnerGenerationProvenance {
                profile,
                profile_authority_sha256,
                provenance,
                ..
            } => Some((profile, profile_authority_sha256, provenance)),
            _ => None,
        })
        .expect("fixture compact wrapper");
    *wrapper.0 = Some(legacy_profile);
    *wrapper.1 = None;
    *wrapper.2 = None;

    let reopened = restore_all(&fixture.editor, history).expect("reopen legacy wrapper");
    let rewritten = reopened
        .export_history_v1(project_id)
        .expect("rewrite admitted history");
    let InverseV1::RestoreBeginnerGenerationProvenance {
        profile,
        profile_authority_sha256,
        provenance,
        ..
    } = &rewritten.undo_stack[1].inverse
    else {
        panic!("rewritten history must keep compact wrapper semantics")
    };
    assert!(profile.is_none());
    assert!(profile_authority_sha256.is_some());
    assert!(provenance.is_some());
}

#[test]
fn compact_wrapper_requires_an_explicit_current_profile_binding() {
    let fixture = underlay_provenance_history_fixture();
    let history = fixture
        .editor
        .export_history_v1(ProjectId::new())
        .expect("export compact fixture");
    assert_eq!(
        EditorState::with_all_document_parts_annotations_underlays_memo_and_history_v1(
            fixture.editor.pattern().clone(),
            fixture.editor.paper().clone(),
            fixture.editor.instruction_timeline().clone(),
            fixture.editor.geometric_constraints().clone(),
            fixture.editor.project_layers().clone(),
            fixture.editor.element_metadata().clone(),
            fixture.editor.annotations().clone(),
            fixture.editor.underlays().clone(),
            fixture.editor.project_memo().to_owned(),
            history,
        )
        .expect_err("a digest cannot synthesize an unbound profile"),
        EditorHistoryErrorV1::InvalidInverse
    );
}

#[test]
fn provenance_wrapper_wire_rejects_mixed_and_empty_authority_encodings() {
    let profile =
        beginner_profile_with_underlay_provenance(UnderlayId::new(), AssetId::new(), 0x43);
    let authority = beginner_design_profile_authority_sha256_v1(&profile);
    let provenance = profile
        .generation_provenance
        .clone()
        .expect("fixture provenance");
    let inner = || {
        Box::new(InverseV1::RestoreProjectMemo {
            memo: "before".to_owned(),
        })
    };
    for wire in [
        InverseV1::RestoreBeginnerGenerationProvenance {
            profile: Some(Box::new(profile)),
            profile_authority_sha256: Some(authority),
            provenance: Some(Box::new(provenance)),
            inner: inner(),
        },
        InverseV1::RestoreBeginnerGenerationProvenance {
            profile: None,
            profile_authority_sha256: None,
            provenance: None,
            inner: inner(),
        },
    ] {
        assert_eq!(
            inverse_from_wire(wire),
            Err(EditorHistoryErrorV1::InvalidInverse)
        );
    }
}

#[test]
fn combined_underlay_profile_inverse_rejects_malformed_and_oversized_documents() {
    let fixture = underlay_provenance_history_fixture();
    let history = fixture
        .editor
        .export_history_v1(ProjectId::new())
        .expect("export authority-invalidating history");

    let mut unsupported_schema = history.clone();
    combined_underlay_profile_inverse_mut(&mut unsupported_schema)
        .0
        .schema_version = 2;
    assert_eq!(
        restore_all(&fixture.editor, unsupported_schema)
            .expect_err("unsupported inverse underlay schema must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut non_finite = history.clone();
    combined_underlay_profile_inverse_mut(&mut non_finite)
        .0
        .underlays[0]
        .opacity = f64::NAN;
    assert_eq!(
        restore_all(&fixture.editor, non_finite)
            .expect_err("non-finite inverse underlay must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut oversized = history;
    let underlay_layer = fixture.target_before.layer;
    combined_underlay_profile_inverse_mut(&mut oversized)
        .0
        .underlays = (0..=MAX_UNDERLAYS_V1)
        .map(|_| underlay_record(UnderlayId::new(), AssetId::new(), underlay_layer))
        .collect();
    assert_eq!(
        restore_all(&fixture.editor, oversized)
            .expect_err("oversized inverse underlay document must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );
}

#[test]
fn combined_underlay_profile_inverse_rejects_target_identity_and_asset_tampering() {
    let fixture = underlay_provenance_history_fixture();
    let history = fixture
        .editor
        .export_history_v1(ProjectId::new())
        .expect("export authority-invalidating history");

    let mut underlay_id_tampered = history.clone();
    combined_underlay_profile_inverse_mut(&mut underlay_id_tampered)
        .0
        .underlays[0]
        .id = UnderlayId::new();
    assert_eq!(
        restore_all(&fixture.editor, underlay_id_tampered)
            .expect_err("tampered inverse underlay identity must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut underlay_asset_tampered = history.clone();
    combined_underlay_profile_inverse_mut(&mut underlay_asset_tampered)
        .0
        .underlays[0]
        .asset = AssetId::new();
    assert_eq!(
        restore_all(&fixture.editor, underlay_asset_tampered)
            .expect_err("tampered inverse underlay asset must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut target_id_tampered = history.clone();
    let (_, profile) = combined_underlay_profile_inverse_mut(&mut target_id_tampered);
    let Some(BeginnerTargetAssetReferenceV1::ReferenceImage { underlay_id, .. }) =
        profile.generation_constraints.target_asset.as_mut()
    else {
        panic!("fixture profile must target a reference image")
    };
    *underlay_id = UnderlayId::new();
    assert_eq!(
        restore_all(&fixture.editor, target_id_tampered)
            .expect_err("tampered profile target underlay identity must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut target_asset_tampered = history;
    let (_, profile) = combined_underlay_profile_inverse_mut(&mut target_asset_tampered);
    let Some(BeginnerTargetAssetReferenceV1::ReferenceImage { asset_id, .. }) =
        profile.generation_constraints.target_asset.as_mut()
    else {
        panic!("fixture profile must target a reference image")
    };
    *asset_id = AssetId::new();
    assert_eq!(
        restore_all(&fixture.editor, target_asset_tampered)
            .expect_err("tampered profile target asset must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );
}

#[test]
fn combined_underlay_profile_inverse_rejects_nil_binding_identifiers() {
    let fixture = underlay_provenance_history_fixture();
    let history = fixture
        .editor
        .export_history_v1(ProjectId::new())
        .expect("export authority-invalidating history");

    let mut nil_record_id = history.clone();
    combined_underlay_profile_inverse_mut(&mut nil_record_id)
        .0
        .underlays[0]
        .id = nil_underlay_id();
    assert_eq!(
        restore_all(&fixture.editor, nil_record_id)
            .expect_err("nil inverse underlay ID must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut nil_record_asset = history.clone();
    combined_underlay_profile_inverse_mut(&mut nil_record_asset)
        .0
        .underlays[0]
        .asset = nil_asset_id();
    assert_eq!(
        restore_all(&fixture.editor, nil_record_asset)
            .expect_err("nil inverse underlay asset must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut nil_record_layer = history.clone();
    combined_underlay_profile_inverse_mut(&mut nil_record_layer)
        .0
        .underlays[0]
        .layer = nil_layer_id();
    assert_eq!(
        restore_all(&fixture.editor, nil_record_layer)
            .expect_err("nil inverse underlay layer must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut nil_target_id = history.clone();
    let (_, profile) = combined_underlay_profile_inverse_mut(&mut nil_target_id);
    let Some(BeginnerTargetAssetReferenceV1::ReferenceImage { underlay_id, .. }) =
        profile.generation_constraints.target_asset.as_mut()
    else {
        panic!("fixture profile must target a reference image")
    };
    *underlay_id = nil_underlay_id();
    assert_eq!(
        restore_all(&fixture.editor, nil_target_id)
            .expect_err("nil profile target underlay ID must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut nil_target_asset = history;
    let (_, profile) = combined_underlay_profile_inverse_mut(&mut nil_target_asset);
    let Some(BeginnerTargetAssetReferenceV1::ReferenceImage { asset_id, .. }) =
        profile.generation_constraints.target_asset.as_mut()
    else {
        panic!("fixture profile must target a reference image")
    };
    *asset_id = nil_asset_id();
    assert_eq!(
        restore_all(&fixture.editor, nil_target_asset)
            .expect_err("nil profile target asset must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );
}

#[test]
fn combined_underlay_profile_inverse_rejects_foreign_and_tampered_live_layers() {
    let fixture = underlay_provenance_history_fixture();
    let history = fixture
        .editor
        .export_history_v1(ProjectId::new())
        .expect("export authority-invalidating history");

    let mut foreign_inverse_layer = history.clone();
    combined_underlay_profile_inverse_mut(&mut foreign_inverse_layer)
        .0
        .underlays[0]
        .layer = DEFAULT_PROJECT_LAYER_ID;
    assert_eq!(
        restore_all(&fixture.editor, foreign_inverse_layer)
            .expect_err("inverse underlay on a crease-pattern layer must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let target_layer = fixture.target_before.layer;
    let mut wrong_kind_current = fixture.editor.clone();
    wrong_kind_current
        .project_layers
        .layers
        .iter_mut()
        .find(|layer| layer.id == target_layer)
        .expect("fixture underlay layer")
        .content_kind = LayerContentKindV1::Annotation;
    assert_eq!(
        restore_all(&wrong_kind_current, history.clone())
            .expect_err("current underlay bound to a wrong-kind layer must be rejected"),
        EditorHistoryErrorV1::InvalidCommand
    );

    let mut missing_current_layer = fixture.editor;
    missing_current_layer
        .project_layers
        .layers
        .retain(|layer| layer.id != target_layer);
    assert_eq!(
        restore_all(&missing_current_layer, history)
            .expect_err("current underlay bound to a missing layer must be rejected"),
        EditorHistoryErrorV1::InvalidCommand
    );
}

#[test]
fn combined_underlay_profile_inverse_requires_reference_provenance_and_rejects_tampering() {
    let fixture = underlay_provenance_history_fixture();
    let history = fixture
        .editor
        .export_history_v1(ProjectId::new())
        .expect("export authority-invalidating history");

    let mut missing_provenance = history.clone();
    combined_underlay_profile_inverse_mut(&mut missing_provenance)
        .1
        .generation_provenance = None;
    assert_eq!(
        restore_all(&fixture.editor, missing_provenance)
            .expect_err("combined inverse without provenance must be rejected"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut reference_model = history.clone();
    combined_underlay_profile_inverse_mut(&mut reference_model)
        .1
        .generation_constraints
        .target_asset = Some(BeginnerTargetAssetReferenceV1::ReferenceModel {
        asset_id: fixture.target_before.asset,
    });
    assert_eq!(
        restore_all(&fixture.editor, reference_model)
            .expect_err("combined inverse requires a reference-image target"),
        EditorHistoryErrorV1::InvalidInverse
    );

    let mut tampered_provenance = history;
    let (_, profile) = combined_underlay_profile_inverse_mut(&mut tampered_provenance);
    profile
        .generation_provenance
        .as_mut()
        .expect("fixture inverse must restore provenance")
        .topology_authority_sha256[0] ^= 0xff;

    assert_eq!(
        restore_all(&fixture.editor, tampered_provenance)
            .expect_err("tampered inverse provenance must be rejected"),
        EditorHistoryErrorV1::InverseMismatch
    );
}

#[test]
fn persisted_profile_updates_reject_provenance_escalation_and_replay_safe_downgrade() {
    let mut unproven_editor = EditorState::new(CreasePattern::empty());
    let mut authored_profile = BeginnerDesignProfileV1::default();
    authored_profile.generation_constraints.maximum_steps += 1;
    unproven_editor
        .execute(
            0,
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(authored_profile),
            },
        )
        .expect("persist an unproven authoring update");
    let mut forged_mint = unproven_editor
        .export_history_v1(ProjectId::new())
        .expect("export unproven profile history");
    let CommandV1::UpdateBeginnerDesignProfile { profile } = &mut forged_mint.undo_stack[0].forward
    else {
        panic!("fixture must persist a profile update")
    };
    profile.generation_provenance =
        beginner_profile_with_underlay_provenance(UnderlayId::new(), AssetId::new(), 0x63)
            .generation_provenance;
    assert_eq!(
        restore(&unproven_editor, forged_mint)
            .expect_err("persisted None-to-Some provenance mint must be rejected"),
        EditorHistoryErrorV1::InvalidCommand
    );

    let fixture = underlay_provenance_history_fixture();
    let mut proven_editor = fixture.editor;
    proven_editor
        .undo(proven_editor.revision())
        .expect("restore generated provenance");
    let proven_profile = proven_editor.beginner_design_profile().clone();
    proven_editor
        .execute(
            proven_editor.revision(),
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(proven_profile.clone()),
            },
        )
        .expect("persist an exact provenance-preserving profile no-op");
    let mut forged_replacement = proven_editor
        .export_history_v1(ProjectId::new())
        .expect("export proven profile history");
    let CommandV1::UpdateBeginnerDesignProfile { profile } = &mut forged_replacement
        .undo_stack
        .last_mut()
        .expect("profile history entry")
        .forward
    else {
        panic!("fixture must end in a profile update")
    };
    profile
        .generation_provenance
        .as_mut()
        .expect("fixture profile has provenance")
        .topology_authority_sha256[0] ^= 0xff;
    assert_eq!(
        restore_all(&proven_editor, forged_replacement)
            .expect_err("persisted Some-to-different-Some replacement must be rejected"),
        EditorHistoryErrorV1::InvalidCommand
    );

    let mut downgrade_editor = proven_editor;
    let mut downgraded_profile = proven_profile.clone();
    downgraded_profile.generation_provenance = None;
    downgrade_editor
        .execute(
            downgrade_editor.revision(),
            Command::UpdateBeginnerDesignProfile {
                profile: Box::new(downgraded_profile.clone()),
            },
        )
        .expect("persist an explicit provenance downgrade");
    let downgrade_history = downgrade_editor
        .export_history_v1(ProjectId::new())
        .expect("export downgrade history");
    let mut reopened = restore_all(&downgrade_editor, downgrade_history)
        .expect("reopen a safe provenance downgrade");
    reopened.undo(0).expect("undo persisted downgrade");
    assert_eq!(reopened.beginner_design_profile(), &proven_profile);
    reopened.redo(1).expect("redo persisted downgrade");
    assert_eq!(reopened.beginner_design_profile(), &downgraded_profile);
}

#[test]
fn multiple_profile_bearing_redo_entries_bind_in_application_order() {
    let UnderlayProvenanceHistoryFixture {
        mut editor,
        target_after,
        ..
    } = underlay_provenance_history_fixture();
    let second_profile =
        beginner_profile_with_underlay_provenance(target_after.id, target_after.asset, 0x52);
    let mut second_generated_timeline = editor.instruction_timeline().clone();
    second_generated_timeline
        .steps
        .push(declarative_instruction_step("Second generated authority"));
    editor
        .execute(
            editor.revision(),
            Command::ApplyBeginnerGeneratedDocument {
                pattern: editor.pattern().clone(),
                paper: editor.paper().clone(),
                instruction_timeline: second_generated_timeline,
                project_layers: editor.project_layers().clone(),
                beginner_design_profile: Box::new(second_profile.clone()),
            },
        )
        .expect("install second generation provenance");
    let mut final_target = target_after;
    final_target.asset = AssetId::new();
    editor
        .execute(
            editor.revision(),
            Command::UpdateUnderlay {
                record: final_target,
            },
        )
        .expect("invalidate second generation provenance");
    let final_underlays = editor.underlays().clone();
    let final_profile = editor.beginner_design_profile().clone();

    editor
        .undo(editor.revision())
        .expect("move second invalidation to Redo");
    editor
        .undo(editor.revision())
        .expect("move second profile update to Redo");
    let history = editor
        .export_history_v1(ProjectId::new())
        .expect("export two-entry Redo history");
    assert_eq!(history.redo_len(), 2);

    let mut reopened =
        restore_all(&editor, history.clone()).expect("validate ordered Redo history");
    reopened.redo(0).expect("redo second profile update");
    assert_eq!(reopened.beginner_design_profile(), &second_profile);
    reopened
        .redo(1)
        .expect("redo second authority invalidation");
    assert_eq!(reopened.underlays(), &final_underlays);
    assert_eq!(reopened.beginner_design_profile(), &final_profile);

    let mut reordered = history;
    reordered.redo_stack.swap(0, 1);
    assert_eq!(
        restore_all(&editor, reordered)
            .expect_err("reordered profile-bearing Redo entries must be rejected"),
        EditorHistoryErrorV1::InverseMismatch
    );
}

#[test]
fn runtime_redo_rejects_a_different_valid_profile_before_mutation() {
    let fixture = underlay_provenance_history_fixture();
    let mut editor = fixture.editor;
    editor
        .undo(editor.revision())
        .expect("restore provenance and create Redo entry");
    let before_underlays = editor.underlays().clone();
    let before_revision = editor.revision();
    let mut different_profile = editor.beginner_design_profile().clone();
    different_profile.generation_constraints.maximum_steps += 1;
    assert!(validate_beginner_design_profile_v1(&different_profile));
    editor
        .restore_beginner_design_profile(different_profile.clone())
        .expect("install different valid load-time profile");

    assert_eq!(
        editor.redo(before_revision),
        Err(CommandError::InvalidBeginnerDesignProfile)
    );
    assert_eq!(editor.revision(), before_revision);
    assert_eq!(editor.underlays(), &before_underlays);
    assert_eq!(editor.beginner_design_profile(), &different_profile);
    assert!(editor.can_redo());
}

#[test]
fn runtime_undo_rejects_a_different_valid_profile_before_mutation() {
    let fixture = underlay_provenance_history_fixture();
    let mut editor = fixture.editor;
    let before_underlays = editor.underlays().clone();
    let before_revision = editor.revision();
    let mut different_profile = editor.beginner_design_profile().clone();
    different_profile.generation_constraints.maximum_steps += 1;
    assert!(validate_beginner_design_profile_v1(&different_profile));
    editor
        .restore_beginner_design_profile(different_profile.clone())
        .expect("install different valid load-time profile");

    assert_eq!(
        editor.undo(before_revision),
        Err(CommandError::InvalidBeginnerDesignProfile)
    );
    assert_eq!(editor.revision(), before_revision);
    assert_eq!(editor.underlays(), &before_underlays);
    assert_eq!(editor.beginner_design_profile(), &different_profile);
    assert!(editor.can_undo());
}

#[test]
fn runtime_undo_does_not_restore_provenance_into_a_missing_reference_binding() {
    let fixture = underlay_provenance_history_fixture();
    let mut editor = fixture.editor;
    editor
        .undo(editor.revision())
        .expect("restore the valid target binding and provenance");
    let source = editor.pattern().vertices[0].clone();
    editor
        .execute(
            editor.revision(),
            Command::MoveVertex {
                id: source.id,
                position: Point2::new(source.position.x + 1.0, source.position.y + 1.0),
            },
        )
        .expect("invalidate provenance through geometry");
    let moved_pattern = editor.pattern().clone();
    editor.restore_underlays(UnderlayDocumentV1::default());
    let before_revision = editor.revision();

    assert_eq!(
        editor.undo(before_revision),
        Err(CommandError::InvalidBeginnerDesignProfile)
    );
    assert_eq!(editor.revision(), before_revision);
    assert_eq!(editor.pattern(), &moved_pattern);
    assert!(
        editor
            .beginner_design_profile()
            .generation_provenance
            .is_none()
    );
}

#[test]
fn maximum_profile_authority_is_stored_once_as_a_compact_digest_per_history_entry() {
    let provenance =
        beginner_profile_with_underlay_provenance(UnderlayId::new(), AssetId::new(), 0x6a)
            .generation_provenance
            .expect("fixture provenance");
    let mut profile = BeginnerDesignProfileV1 {
        generation_provenance: Some(provenance.clone()),
        ..BeginnerDesignProfileV1::default()
    };
    profile.generation_constraints.bulge_targets = (1_u16..=32)
        .map(|id| BeginnerBulgeTargetV1 {
            id,
            face_ids: vec![FaceId::new()],
            range_min_tenths_mm: [-10, -10, -10],
            range_max_tenths_mm: [10, 10, 10],
            direction_milli: [0, 0, 1_000],
            amount_tenths_mm: 50,
            source_fold_model_fingerprint: "a".repeat(64),
            reference_surface_binding: Some(BeginnerReferenceSurfaceBindingV1 {
                asset_id: AssetId::new(),
                range_id: id,
                protrusion_id: id,
                triangle_indices: (0..40_000).collect(),
                range_digest_sha256: [id as u8; 32],
            }),
        })
        .collect();
    assert!(validate_beginner_design_profile_v1(&profile));
    let profile_bytes = serde_json::to_vec(&profile)
        .expect("encode maximum valid profile")
        .len();
    let authority = beginner_design_profile_authority_sha256_v1(&profile);
    let entry = HistoryEntryV1 {
        forward: CommandV1::UpdateProjectMemo {
            memo: "after".to_owned(),
        },
        inverse: InverseV1::RestoreBeginnerGenerationProvenance {
            profile: None,
            profile_authority_sha256: Some(authority),
            provenance: Some(Box::new(provenance)),
            inner: Box::new(InverseV1::RestoreProjectMemo {
                memo: "before".to_owned(),
            }),
        },
        speculative_unproven_fold_v1: None,
    };
    let one = history_with(128, vec![entry.clone()], Vec::new());
    let maximum = history_with(128, vec![entry; 128], Vec::new());
    assert_eq!(maximum.validate_shape(), Ok(128));
    let one_bytes = serde_json::to_vec(&one).expect("encode one entry").len();
    let maximum_bytes = serde_json::to_vec(&maximum)
        .expect("encode maximum history")
        .len();

    assert!(
        maximum_bytes <= one_bytes * 128,
        "history storage must grow only linearly with compact entries"
    );
    assert!(
        maximum_bytes < profile_bytes / 4,
        "history unexpectedly retained the maximum profile: history={maximum_bytes}, profile={profile_bytes}"
    );
}
