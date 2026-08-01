use super::super::super::ProjectState;
use super::*;

// The tetrahedral dihedral angle makes the three signed hinge rotations below
// compose to a 180-degree in-plane rotation.  The first and fourth triangles
// are therefore parallel while remaining separated in Z.  Two-ULP linear
// segments have an exactly representable midpoint and retain that real overlap
// relation without widening any production tolerance.
const NON_FLAT_SOURCE_ANGLE_DEGREES_V1: f64 = 109.471_220_634_490_69;
const NON_FLAT_FIRST_TARGET_ANGLE_DEGREES_V1: f64 = 109.471_220_634_490_72;
const NON_FLAT_SECOND_TARGET_ANGLE_DEGREES_V1: f64 = 109.471_220_634_490_75;

fn fixed_id<T: serde::de::DeserializeOwned>(group: &str, index: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-4000-{group}-{index:012x}\"")).unwrap()
}

fn three_hinge_non_flat_project_v1() -> (ProjectState, Vec<ori_domain::EdgeId>) {
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, Vertex};

    // Four triangles in a material Tree:
    //
    //        F
    //       / \
    //  A---C   D
    //   \ / \ /
    //    B---/
    //     \ E
    //
    // The actual outer walk is E-A-C-F-D-B.  AB, BC, and CD are the only
    // hinges.  Their signed axes are 0, 60, and 120 degrees respectively.
    // E and F deliberately make the separated first/fourth folded projections
    // overlap with positive area.
    let points = [
        (0.0, 0.0),                     // A
        (100.0, 0.0),                   // B
        (150.0, 86.602_540_378_443_86), // C
        (200.0, 0.0),                   // D
        (200.0, -100.0),                // E
        (250.0, 150.0),                 // F
    ];
    let vertices = points
        .into_iter()
        .enumerate()
        .map(|(index, (x, y))| Vertex {
            id: fixed_id("8a10", index as u64 + 1),
            position: Point2::new(x, y),
        })
        .collect::<Vec<_>>();
    let boundary = [4_usize, 0, 2, 5, 3, 1]
        .into_iter()
        .map(|index| vertices[index].id)
        .collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: fixed_id("8a20", index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let hinges = vec![
        fixed_id("8a20", 20),
        fixed_id("8a20", 21),
        fixed_id("8a20", 22),
    ];
    edges.extend([
        Edge {
            id: hinges[0],
            start: vertices[0].id,
            end: vertices[1].id,
            // The fixed first triangle is on the right of A->B, so Valley
            // yields the required positive signed rotation.
            kind: EdgeKind::Valley,
        },
        Edge {
            id: hinges[1],
            start: vertices[1].id,
            end: vertices[2].id,
            kind: EdgeKind::Mountain,
        },
        Edge {
            id: hinges[2],
            start: vertices[2].id,
            end: vertices[3].id,
            // C->D points at -60 degrees.  Its negative traversal rotation is
            // exactly the positive rotation about the 120-degree axis.
            kind: EdgeKind::Mountain,
        },
    ]);
    let first_boundary_edge = edges[0].id;
    let mut project = ProjectState::new_with_paper(
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices: boundary,
            thickness_mm: 0.1,
            ..Paper::default()
        },
    );
    project.instance_id = fixed_id("8a30", 1);
    project.project_id = fixed_id("8a30", 2);
    let topology = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let snapshot = topology.simulation_snapshot().unwrap();
    assert_eq!(snapshot.faces.len(), 4);
    assert_eq!(snapshot.hinge_adjacency.len(), 3);
    let fixed_face = snapshot
        .faces
        .iter()
        .find(|face| {
            face.outer
                .half_edges
                .iter()
                .any(|half_edge| half_edge.edge == first_boundary_edge)
        })
        .expect("the outer E-A edge belongs to the first triangle")
        .id;
    super::super::super::applied_pose::tests::install_pose_authority_with_angles(
        &mut project,
        hinges
            .iter()
            .copied()
            .map(|edge| (edge, NON_FLAT_SOURCE_ANGLE_DEGREES_V1))
            .collect(),
        fixed_face,
    )
    .unwrap();
    let angles = CanonicalHingeAngles::new(
        hinges
            .iter()
            .copied()
            .map(|edge| HingeAngle::new(edge, NON_FLAT_SOURCE_ANGLE_DEGREES_V1).unwrap())
            .collect(),
    )
    .unwrap();
    let flat = freshly_analyze_flat_layer_order_v1(
        project.project_id,
        project.editor.revision(),
        project.editor.pattern(),
        project.editor.paper(),
    )
    .unwrap();
    let evidence = revalidate_current_non_flat_layer_order_v1(
        project.project_id,
        project.editor.revision(),
        project.editor.pattern(),
        project.editor.paper(),
        Some(fixed_face),
        &angles,
        &flat,
        6,
    )
    .unwrap();
    assert!(
        evidence.face_pair_order_count() > 0,
        "the positive family must transport a real non-flat pair order"
    );
    assert!(
        evidence.overlap_cell_count() > 0
            && evidence
                .overlap_cells()
                .iter()
                .all(|cell| cell.exact_boundary().len() >= 3),
        "the pair order must be backed by a non-empty exact overlap cell"
    );
    project.current_layer_evidence = Some(CurrentLayerEvidence::NonFlat(evidence));
    (project, hinges)
}

fn preview_request_v1(
    project: &ProjectState,
    hinges: &[ori_domain::EdgeId],
    target: f64,
) -> NonFlatCycleContinuationPreviewRequestV1 {
    NonFlatCycleContinuationPreviewRequestV1 {
        expected_project_instance_id: project.instance_id,
        expected_project_id: project.project_id,
        expected_revision: project.editor.revision(),
        target_angles: hinges
            .iter()
            .copied()
            .map(|edge| NonFlatCycleContinuationAngleV1 {
                edge,
                angle_degrees: target,
            })
            .collect(),
    }
}

fn apply_request_v1(
    preview: &NonFlatCycleContinuationPreviewResponseV1,
) -> ApplyNonFlatCycleContinuationRequestV1 {
    ApplyNonFlatCycleContinuationRequestV1 {
        preview_token: preview.preview_token,
        expected_project_instance_id: preview.project_instance_id,
        expected_project_id: preview.project_id,
        expected_revision: preview.source_revision,
        expected_target_pose_sha256: preview.target_pose_sha256.clone(),
        expected_authority_binding_sha256: preview.authority_binding_sha256.clone(),
    }
}

#[test]
fn non_flat_three_hinge_continuation_chains_twice_with_exact_source_anchor() {
    let (project, hinges) = three_hinge_non_flat_project_v1();
    let app_state = AppState::new(project);
    let foldability_state = GlobalFlatFoldabilityState::default();
    let state = NonFlatCycleContinuationState::default();

    for target in [
        NON_FLAT_FIRST_TARGET_ANGLE_DEGREES_V1,
        NON_FLAT_SECOND_TARGET_ANGLE_DEGREES_V1,
    ] {
        let request = {
            let project = lock_project(&app_state).unwrap();
            preview_request_v1(&project, &hinges, target)
        };
        let preview = mint_non_flat_cycle_continuation_inner_v1(
            &app_state,
            &state,
            request,
            NonFlatCycleContinuationLimitsV1 {
                max_face_pairs: 6,
                transport: NonFlatCellTransportLimitsV1 {
                    max_faces: 4,
                    max_cells: 6,
                    max_pairs: 6,
                    max_boundary_points: 192,
                },
            },
        )
        .expect("exact non-flat continuation authority");
        {
            let slot = state.0.lock().unwrap();
            let record = slot.as_ref().unwrap();
            assert_eq!(record.token, preview.preview_token);
            let source = record.authority.generated.schedule().evaluate(0.0).unwrap();
            let target = record.authority.generated.schedule().evaluate(1.0).unwrap();
            assert!(exact_hinge_angles_match_v1(
                source.as_slice(),
                record.authority.source.hinge_angles()
            ));
            assert!(exact_hinge_angles_match_v1(
                target.as_slice(),
                record.authority.target.hinge_angles()
            ));
            assert_eq!(
                preview.source_pose_sha256,
                lowercase_hex_v1(pose_state_fingerprint_v1(&source))
            );
            assert_eq!(
                preview.target_pose_sha256,
                lowercase_hex_v1(pose_state_fingerprint_v1(&target))
            );
        }
        assert!(preview.continuous_path_certified);
        assert!(preview.non_flat_cell_transport_certified);
        assert!(preview.transported_cell_count > 0);
        assert!(preview.transported_pair_count > 0);
        let applied = apply_non_flat_cycle_continuation_inner_v1(
            &app_state,
            &foldability_state,
            &state,
            apply_request_v1(&preview),
        )
        .expect("atomic non-flat continuation Apply");
        assert_eq!(applied, preview.target_revision);
        let project = lock_project(&app_state).unwrap();
        let CurrentLayerEvidence::NonFlat(evidence) =
            project.current_layer_evidence.as_ref().unwrap()
        else {
            panic!("continuation must retain the exact target evidence");
        };
        assert_eq!(evidence.target_revision(), applied);
        assert!(
            evidence
                .hinge_angles()
                .iter()
                .all(|angle| angle.angle_degrees().to_bits() == target.to_bits())
        );
        assert!(
            project
                .trusted_path_certificates
                .export_attestation_v1(
                    project.instance_id,
                    project.project_id,
                    project.editor.instruction_timeline(),
                )
                .expect("live non-flat continuation registry")
                .is_some(),
            "a successful continuation must be immediately export-attestable"
        );
    }
    let project = lock_project(&app_state).unwrap();
    assert_eq!(project.editor.revision(), 2);
    let steps = &project.editor.instruction_timeline().steps;
    assert_eq!(steps.len(), 4);
    assert_eq!(
        steps[1].pose, steps[2].pose,
        "the second native continuation must begin at the first exact target"
    );
}

#[test]
fn non_flat_continuation_exact_limits_pass_and_every_one_short_fails_closed() {
    let (project, hinges) = three_hinge_non_flat_project_v1();
    let app_state = AppState::new(project);
    let state = NonFlatCycleContinuationState::default();
    let request = {
        let project = lock_project(&app_state).unwrap();
        preview_request_v1(&project, &hinges, NON_FLAT_FIRST_TARGET_ANGLE_DEGREES_V1)
    };
    let broad = mint_non_flat_cycle_continuation_inner_v1(
        &app_state,
        &state,
        request,
        NonFlatCycleContinuationLimitsV1::default(),
    )
    .expect("broad limits must reveal the exact retained counts");
    let exact = {
        let slot = state.0.lock().unwrap();
        let record = slot.as_ref().unwrap();
        let layers = [&record.authority.source, &record.authority.target];
        NonFlatCycleContinuationLimitsV1 {
            max_face_pairs: layers
                .iter()
                .map(|layer| layer.tested_face_pairs())
                .max()
                .unwrap(),
            transport: NonFlatCellTransportLimitsV1 {
                max_faces: layers
                    .iter()
                    .flat_map(|layer| [layer.material_faces().len(), layer.folded_faces().len()])
                    .max()
                    .unwrap(),
                max_cells: layers
                    .iter()
                    .map(|layer| layer.overlap_cells().len())
                    .max()
                    .unwrap(),
                max_pairs: layers
                    .iter()
                    .map(|layer| layer.face_pair_orders().len())
                    .max()
                    .unwrap(),
                max_boundary_points: layers
                    .iter()
                    .map(|layer| {
                        layer
                            .overlap_cells()
                            .iter()
                            .map(|cell| cell.exact_boundary().len())
                            .sum()
                    })
                    .max()
                    .unwrap(),
            },
        }
    };
    cancel_non_flat_cycle_continuation_inner_v1(&state, broad.preview_token).unwrap();

    let request = {
        let project = lock_project(&app_state).unwrap();
        preview_request_v1(&project, &hinges, NON_FLAT_FIRST_TARGET_ANGLE_DEGREES_V1)
    };
    let preview = mint_non_flat_cycle_continuation_inner_v1(&app_state, &state, request, exact)
        .expect("every exact limit is admitted");
    cancel_non_flat_cycle_continuation_inner_v1(&state, preview.preview_token).unwrap();

    let mut one_short = Vec::new();
    if exact.max_face_pairs > 0 {
        let mut limits = exact;
        limits.max_face_pairs -= 1;
        one_short.push(limits);
    }
    for field in 0..4 {
        let mut limits = exact;
        let value = match field {
            0 => &mut limits.transport.max_faces,
            1 => &mut limits.transport.max_cells,
            2 => &mut limits.transport.max_pairs,
            _ => &mut limits.transport.max_boundary_points,
        };
        if *value > 0 {
            *value -= 1;
            one_short.push(limits);
        }
    }
    assert_eq!(one_short.len(), 5);
    for limits in one_short {
        let request = {
            let project = lock_project(&app_state).unwrap();
            preview_request_v1(&project, &hinges, NON_FLAT_FIRST_TARGET_ANGLE_DEGREES_V1)
        };
        assert_eq!(
            mint_non_flat_cycle_continuation_inner_v1(&app_state, &state, request, limits,)
                .unwrap_err(),
            CYCLE_PATH_RESOURCE_MESSAGE
        );
    }
}

#[test]
fn non_flat_continuation_rejects_binding_tamper_and_pose_generation_aba() {
    let (project, hinges) = three_hinge_non_flat_project_v1();
    let app_state = AppState::new(project);
    let foldability_state = GlobalFlatFoldabilityState::default();
    let state = NonFlatCycleContinuationState::default();
    let request = {
        let project = lock_project(&app_state).unwrap();
        preview_request_v1(&project, &hinges, NON_FLAT_FIRST_TARGET_ANGLE_DEGREES_V1)
    };
    let preview = mint_non_flat_cycle_continuation_inner_v1(
        &app_state,
        &state,
        request,
        NonFlatCycleContinuationLimitsV1::default(),
    )
    .unwrap();
    let mut tampered = apply_request_v1(&preview);
    let replacement = if tampered.expected_authority_binding_sha256.starts_with("00") {
        "ff"
    } else {
        "00"
    };
    tampered
        .expected_authority_binding_sha256
        .replace_range(0..2, replacement);
    assert!(
        apply_non_flat_cycle_continuation_inner_v1(
            &app_state,
            &foldability_state,
            &state,
            tampered,
        )
        .is_err()
    );
    assert_eq!(lock_project(&app_state).unwrap().editor.revision(), 0);
    assert!(
        apply_non_flat_cycle_continuation_inner_v1(
            &app_state,
            &foldability_state,
            &state,
            apply_request_v1(&preview),
        )
        .is_err(),
        "a failed matching Apply attempt must consume the token"
    );

    let request = {
        let project = lock_project(&app_state).unwrap();
        preview_request_v1(&project, &hinges, NON_FLAT_FIRST_TARGET_ANGLE_DEGREES_V1)
    };
    let preview = mint_non_flat_cycle_continuation_inner_v1(
        &app_state,
        &state,
        request,
        NonFlatCycleContinuationLimitsV1::default(),
    )
    .unwrap();
    let mut malformed = apply_request_v1(&preview);
    malformed.expected_target_pose_sha256.pop();
    assert!(
        apply_non_flat_cycle_continuation_inner_v1(
            &app_state,
            &foldability_state,
            &state,
            malformed,
        )
        .is_err()
    );
    assert!(
        apply_non_flat_cycle_continuation_inner_v1(
            &app_state,
            &foldability_state,
            &state,
            apply_request_v1(&preview),
        )
        .is_err(),
        "a malformed matching Apply attempt must also consume the token"
    );

    let request = {
        let project = lock_project(&app_state).unwrap();
        preview_request_v1(&project, &hinges, NON_FLAT_FIRST_TARGET_ANGLE_DEGREES_V1)
    };
    let preview = mint_non_flat_cycle_continuation_inner_v1(
        &app_state,
        &state,
        request,
        NonFlatCycleContinuationLimitsV1::default(),
    )
    .unwrap();

    {
        let mut project = lock_project(&app_state).unwrap();
        let fixed = project
            .editor
            .current_applied_pose()
            .and_then(|pose| pose.fixed_face())
            .unwrap();
        super::super::super::applied_pose::tests::install_pose_authority_with_angles(
            &mut project,
            hinges
                .iter()
                .copied()
                .map(|edge| (edge, NON_FLAT_SOURCE_ANGLE_DEGREES_V1))
                .collect(),
            fixed,
        )
        .unwrap();
    }
    assert!(
        apply_non_flat_cycle_continuation_inner_v1(
            &app_state,
            &foldability_state,
            &state,
            apply_request_v1(&preview),
        )
        .is_err(),
        "same-value pose reissue is an ABA and must invalidate the token"
    );
    assert_eq!(lock_project(&app_state).unwrap().editor.revision(), 0);
}

#[test]
fn non_flat_continuation_pose_reissue_failure_rolls_back_the_complete_project() {
    let (project, hinges) = three_hinge_non_flat_project_v1();
    let app_state = AppState::new(project);
    let foldability_state = GlobalFlatFoldabilityState::default();
    let state = NonFlatCycleContinuationState::default();
    let (document_before, layer_before, pose_before, registry_len_before) = {
        let project = lock_project(&app_state).unwrap();
        (
            project.document(),
            project.current_layer_evidence.clone(),
            project
                .applied_pose_authority
                .capture_capability(&project)
                .unwrap()
                .unwrap(),
            project.trusted_path_certificates.len_v1(),
        )
    };
    let request = {
        let project = lock_project(&app_state).unwrap();
        preview_request_v1(&project, &hinges, NON_FLAT_FIRST_TARGET_ANGLE_DEGREES_V1)
    };
    let preview = mint_non_flat_cycle_continuation_inner_v1(
        &app_state,
        &state,
        request,
        NonFlatCycleContinuationLimitsV1::default(),
    )
    .unwrap();
    let _failure_guard = fail_next_non_flat_pose_reissue_for_test_v1();
    assert!(
        apply_non_flat_cycle_continuation_inner_v1(
            &app_state,
            &foldability_state,
            &state,
            apply_request_v1(&preview),
        )
        .is_err()
    );
    let project = lock_project(&app_state).unwrap();
    assert_eq!(project.document(), document_before);
    assert_eq!(project.current_layer_evidence, layer_before);
    assert_eq!(
        project.trusted_path_certificates.len_v1(),
        registry_len_before,
        "a post-mutation rollback must retain the old registry image"
    );
    assert!(
        project
            .applied_pose_authority
            .revalidate_capability(&project, &pose_before)
            .unwrap()
            .is_some(),
        "rollback must restore the exact source pose generation"
    );
}
