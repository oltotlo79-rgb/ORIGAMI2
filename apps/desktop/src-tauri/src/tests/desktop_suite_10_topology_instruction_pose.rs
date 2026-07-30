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
