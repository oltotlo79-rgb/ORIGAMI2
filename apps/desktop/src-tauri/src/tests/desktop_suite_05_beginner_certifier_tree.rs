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
            Command::ApplyBeginnerGeneratedDocument {
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
