{
        let proof = match project.current_layer_evidence.as_ref() {
            Some(super::super::stacked_fold_transaction::CurrentLayerEvidence::NonFlat(proof)) => {
                proof
            }
            _ => panic!("atomic apply must install its non-flat layer-order evidence"),
        };
        assert_eq!(proof.identity_namespace(), project.project_id);
        assert_eq!(proof.target_revision(), applied_revision);
        assert_eq!(
            proof.target_fingerprint().to_hex(),
            project.editor.fold_model_fingerprint_v1()
        );
        let active_pose = project
            .editor
            .current_applied_pose()
            .expect("the genuine Apply installs one complete semantic pose");
        assert_eq!(active_pose.model_id(), expected_pose_model_id);
        assert_eq!(active_pose.fixed_face(), proof.fixed_face());
        assert_eq!(active_pose.hinge_angles().len(), proof.hinge_angles().len());
        assert!(
            active_pose
                .hinge_angles()
                .iter()
                .zip(proof.hinge_angles())
                .all(|(pose, proof)| {
                    pose.edge() == proof.edge()
                        && pose.angle_degrees().to_bits() == proof.angle_degrees().to_bits()
                })
        );
        let predecessor = project
            .editor
            .clone_predecessor_if_last_stacked_fold_v1()
            .expect("the genuine Apply exposes one detached predecessor");
        let proof_angles = ori_kinematics::CanonicalHingeAngles::new(proof.hinge_angles().to_vec())
            .expect("the installed proof stores canonical target angles");
        let archived_pairs = proof
            .face_pair_orders()
            .iter()
            .map(|pair| ori_core::ArchivedNonFlatFacePairOrderInputV1 {
                lower_face: pair.lower_face(),
                upper_face: pair.upper_face(),
            })
            .collect::<Vec<_>>();
        let prepared = ori_core::prepare_archived_refined_non_flat_layer_order_v1(
            ori_core::PrepareArchivedRefinedNonFlatLayerOrderRequestV1 {
                identity_namespace: project.project_id,
                source_revision: predecessor.revision(),
                source_pattern: predecessor.pattern(),
                source_paper: predecessor.paper(),
                target_admission_revision: project.editor.revision(),
                target_pattern: project.editor.pattern(),
                target_paper: project.editor.paper(),
                fixed_face: proof.fixed_face(),
                hinge_angles: &proof_angles,
                archived_pair_orders: &archived_pairs,
                lineage_limits: ori_core::FaceLineageLimits::default(),
                geometry_limits: ori_core::StackedFoldGeometryLimitsV1::default(),
                max_face_pairs: ori_core::DEFAULT_MAX_STACKED_FOLD_NON_FLAT_FACE_PAIRS,
            },
        )
        .expect("derive genuine coincident-descendant source constraints");
        assert!(
            !prepared.required_source_pair_orders().is_empty(),
            "the genuine graph fixture must exercise a constrained predecessor solve"
        );
        let constrained_source_flat = super::super::global_flat_foldability::
            reanalyze_editor_flat_layer_order_with_required_pairs(
                project.project_id,
                &predecessor,
                prepared.required_source_pair_orders(),
            )
            .expect("solve every mapped predecessor direction");
        let directly_rebound = ori_core::finish_archived_refined_non_flat_layer_order_v1(
            prepared,
            &constrained_source_flat,
            project
                .editor
                .current_applied_pose()
                .expect("the genuine graph target keeps its semantic pose"),
        )
        .expect("finish the constrained genuine graph proof");
        assert_eq!(
            directly_rebound.face_pair_orders(),
            proof.face_pair_orders()
        );
        assert_eq!(
            directly_rebound.target_revision(),
            project.editor.revision()
        );

        let immediate_archive = project
            .project_archive()
            .expect("serialize immediately applied non-flat evidence");
        let archived_evidence = immediate_archive
            .layer_evidence
            .as_ref()
            .expect("immediate archive must contain non-flat evidence");
        assert!(matches!(
            &archived_evidence.evidence,
            ori_formats::LayerEvidenceArchiveKindV1::NonFlat { .. }
        ));
        assert_eq!(
            serde_json::from_value::<ProjectId>(serde_json::Value::String(
                archived_evidence.project_instance_id.clone()
            ))
            .unwrap(),
            project.instance_id
        );
        assert_eq!(
            serde_json::from_value::<ProjectId>(serde_json::Value::String(
                archived_evidence.project_id.clone()
            ))
            .unwrap(),
            project.project_id
        );
        assert_eq!(archived_evidence.revision, 0);
        assert_eq!(
            archived_evidence.fold_model_fingerprint_sha256,
            project.editor.fold_model_fingerprint_v1()
        );
        let project_signature_before_tamper = (
            project.instance_id,
            project.project_id,
            project.editor.revision(),
            project.editor.fold_model_fingerprint_v1(),
            proof.face_pair_order_count(),
        );
        let assert_archive_rejected = |archive: ori_formats::Ori2ProjectArchive,
                                       case_name: &str| {
            assert!(
                super::super::ProjectState::from_project_archive(
                    archive,
                    std::path::PathBuf::from(format!("split-hinge-cycle-tamper-{case_name}.ori2")),
                )
                .is_err(),
                "the genuine authenticated archive must reject {case_name}"
            );
        };

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat { hinge_angles, .. } =
            &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        assert!(hinge_angles.len() > 1);
        hinge_angles.swap(0, 1);
        assert_archive_rejected(tampered, "noncanonical-hinge-order");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat { hinge_angles, .. } =
            &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        hinge_angles[1] = hinge_angles[0].clone();
        assert_archive_rejected(tampered, "duplicate-hinge");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat { hinge_angles, .. } =
            &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        hinge_angles[0].edge = serde_json::to_value(ori_domain::EdgeId::new())
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        assert_archive_rejected(tampered, "unknown-hinge");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat { hinge_angles, .. } =
            &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        hinge_angles[0].angle_degrees = -0.0;
        assert_archive_rejected(tampered, "negative-zero-hinge-angle");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat { hinge_angles, .. } =
            &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        let bits = hinge_angles[0].angle_degrees.to_bits();
        hinge_angles[0].angle_degrees = f64::from_bits(if bits == 0 { 1 } else { bits - 1 });
        assert_archive_rejected(tampered, "one-ulp-hinge-angle");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat {
            fixed_face,
            material_faces,
            ..
        } = &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        let alternate_fixed = material_faces
            .iter()
            .find(|face| Some(face.face_id.as_str()) != fixed_face.as_deref())
            .expect("the graph fixture has another material face")
            .face_id
            .clone();
        *fixed_face = Some(alternate_fixed);
        assert_archive_rejected(tampered, "fixed-face");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat { material_faces, .. } =
            &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        let replacement = if material_faces[0].face_key_sha256.starts_with('0') {
            "1"
        } else {
            "0"
        };
        material_faces[0]
            .face_key_sha256
            .replace_range(0..1, replacement);
        assert_archive_rejected(tampered, "material-face-key");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat { cells, .. } =
            &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        assert!(!cells.is_empty());
        let coordinate = &mut cells[0].boundary_xy[0][0];
        *coordinate = f64::from_bits(coordinate.to_bits() + 1);
        assert_archive_rejected(tampered, "cell-boundary-bit");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat { pair_orders, .. } =
            &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        assert!(!pair_orders.is_empty());
        let pair = &mut pair_orders[0];
        std::mem::swap(&mut pair.lower_face, &mut pair.upper_face);
        assert_archive_rejected(tampered, "reversed-pair-direction");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat { pair_orders, .. } =
            &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        pair_orders.pop();
        assert_archive_rejected(tampered, "missing-pair");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat { pair_orders, .. } =
            &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        pair_orders.push(pair_orders[0].clone());
        assert_archive_rejected(tampered, "duplicate-pair");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat { pair_orders, .. } =
            &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        pair_orders[0].lower_face = serde_json::to_value(ori_domain::FaceId::new())
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        assert_archive_rejected(tampered, "unknown-pair-face");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat { pair_orders, .. } =
            &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        pair_orders[0].upper_face = pair_orders[0].lower_face.clone();
        assert_archive_rejected(tampered, "equal-pair-face");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat {
            material_faces,
            pair_orders,
            ..
        } = &mut tampered.layer_evidence.as_mut().unwrap().evidence
        else {
            unreachable!()
        };
        let ordered_pairs = pair_orders
            .iter()
            .map(|pair| {
                if pair.lower_face < pair.upper_face {
                    (pair.lower_face.clone(), pair.upper_face.clone())
                } else {
                    (pair.upper_face.clone(), pair.lower_face.clone())
                }
            })
            .collect::<std::collections::HashSet<_>>();
        let non_overlap = material_faces
            .iter()
            .enumerate()
            .find_map(|(first, lower)| {
                material_faces.iter().skip(first + 1).find_map(|upper| {
                    let key = if lower.face_id < upper.face_id {
                        (lower.face_id.clone(), upper.face_id.clone())
                    } else {
                        (upper.face_id.clone(), lower.face_id.clone())
                    };
                    (!ordered_pairs.contains(&key)).then_some(key)
                })
            });
        let (lower_face, upper_face) =
            non_overlap.expect("the genuine graph has a non-overlapping material pair");
        pair_orders.push(ori_formats::LayerEvidencePairOrderV1 {
            lower_face,
            upper_face,
        });
        assert_archive_rejected(tampered, "non-overlapping-extra-pair");

        let mut tampered = immediate_archive.clone();
        tampered.editor_history = None;
        assert_archive_rejected(tampered, "missing-authenticated-history");

        let mut tampered = immediate_archive.clone();
        let ori_formats::LayerEvidenceArchiveKindV1::NonFlat {
            fixed_face,
            material_faces,
            ..
        } = tampered
            .layer_evidence
            .as_ref()
            .map(|evidence| &evidence.evidence)
            .unwrap()
        else {
            unreachable!()
        };
        let current_fixed = fixed_face.as_deref();
        let alternate_fixed = material_faces
            .iter()
            .find(|face| Some(face.face_id.as_str()) != current_fixed)
            .unwrap()
            .face_id
            .clone();
        tampered.document.current_pose.as_mut().unwrap().fixed_face =
            Some(serde_json::from_value(serde_json::Value::String(alternate_fixed)).unwrap());
        assert_archive_rejected(tampered, "document-current-pose-fixed-face");

        let mut redo_project = super::super::ProjectState::from_project_archive(
            immediate_archive.clone(),
            std::path::PathBuf::from("split-hinge-cycle-redo-branch-source.ori2"),
        )
        .expect("prepare a genuine reopened project for the redo-branch regression");
        let mut reinserted_evidence = redo_project
            .archived_layer_evidence()
            .unwrap()
            .expect("capture evidence before creating the redo branch");
        redo_project
            .editor
            .execute(
                redo_project.editor.revision(),
                ori_core::Command::UpdateProjectMemo {
                    memo: "temporary branch".to_owned(),
                },
            )
            .unwrap();
        redo_project
            .editor
            .undo(redo_project.editor.revision())
            .unwrap();
        assert!(redo_project.editor.can_redo());
        assert!(
            redo_project
                .editor
                .clone_predecessor_if_last_stacked_fold_v1()
                .is_none(),
            "the pending redo branch itself is the authenticated-history rejection reason"
        );
        redo_project.current_layer_evidence = None;
        reinserted_evidence.revision = 0;
        reinserted_evidence.fold_model_fingerprint_sha256 =
            redo_project.editor.fold_model_fingerprint_v1();
        let mut redo_archive = redo_project.project_archive().unwrap();
        let redo_control = super::super::ProjectState::from_project_archive(
            redo_archive.clone(),
            std::path::PathBuf::from("split-hinge-cycle-redo-branch-control.ori2"),
        )
        .expect("the same pending-redo archive is valid without reinserted evidence");
        assert_eq!(redo_control.editor.revision(), 0);
        assert!(redo_control.editor.can_redo());
        assert!(
            redo_control
                .editor
                .clone_predecessor_if_last_stacked_fold_v1()
                .is_none()
        );
        redo_archive.layer_evidence = Some(reinserted_evidence);
        assert_archive_rejected(redo_archive, "pending-redo-branch");

        assert_eq!(
            (
                project.instance_id,
                project.project_id,
                project.editor.revision(),
                project.editor.fold_model_fingerprint_v1(),
                proof.face_pair_order_count(),
            ),
            project_signature_before_tamper,
            "all failed admissions leave the live genuine project untouched"
        );

        let mut reopened = super::super::ProjectState::from_project_archive(
            immediate_archive,
            std::path::PathBuf::from("split-hinge-cycle-evidence.ori2"),
        )
        .expect("reopen immediately applied non-flat evidence");
        assert_ne!(reopened.instance_id, project.instance_id);
        assert_eq!(reopened.project_id, project.project_id);
        let reopened_proof = match reopened.current_layer_evidence.as_ref() {
            Some(super::super::stacked_fold_transaction::CurrentLayerEvidence::NonFlat(proof)) => {
                proof
            }
            _ => panic!("fresh open must revalidate non-flat layer-order evidence"),
        };
        assert_eq!(reopened_proof.identity_namespace(), reopened.project_id);
        assert_eq!(reopened_proof.target_revision(), reopened.editor.revision());
        assert_eq!(
            reopened_proof.target_fingerprint().to_hex(),
            reopened.editor.fold_model_fingerprint_v1()
        );
        let reopened_pose = reopened
            .editor
            .current_applied_pose()
            .expect("fresh open restores the complete semantic pose");
        assert_eq!(reopened_pose.model_id(), expected_pose_model_id);
        assert_eq!(reopened_pose.fixed_face(), reopened_proof.fixed_face());
        assert_eq!(
            reopened_pose.hinge_angles().len(),
            reopened_proof.hinge_angles().len()
        );
        assert!(
            reopened_pose
                .hinge_angles()
                .iter()
                .zip(reopened_proof.hinge_angles())
                .all(|(pose, proof)| {
                    pose.edge() == proof.edge()
                        && pose.angle_degrees().to_bits() == proof.angle_degrees().to_bits()
                })
        );
        let reopened_evidence = reopened
            .archived_layer_evidence()
            .expect("serialize freshly revalidated non-flat evidence")
            .expect("fresh archive must contain non-flat evidence");
        assert_eq!(
            serde_json::from_value::<ProjectId>(serde_json::Value::String(
                reopened_evidence.project_instance_id
            ))
            .unwrap(),
            reopened.instance_id
        );
        assert_eq!(reopened_evidence.revision, reopened.editor.revision());
        assert_eq!(
            reopened_evidence.fold_model_fingerprint_sha256,
            reopened.editor.fold_model_fingerprint_v1()
        );
        let second_archive = reopened
            .project_archive()
            .expect("rearchive freshly revalidated non-flat evidence");
        assert!(second_archive.layer_evidence.is_some());
        let second_reopened = super::super::ProjectState::from_project_archive(
            second_archive,
            std::path::PathBuf::from("split-hinge-cycle-evidence-second.ori2"),
        )
        .expect("reopen freshly revalidated non-flat evidence a second time");
        assert_ne!(second_reopened.instance_id, reopened.instance_id);
        assert_eq!(second_reopened.project_id, reopened.project_id);
        let second_proof = match second_reopened.current_layer_evidence.as_ref() {
            Some(super::super::stacked_fold_transaction::CurrentLayerEvidence::NonFlat(proof)) => {
                proof
            }
            _ => panic!("second fresh open must revalidate non-flat layer-order evidence"),
        };
        assert_eq!(
            second_proof.target_revision(),
            second_reopened.editor.revision()
        );
        assert_eq!(
            second_proof.target_fingerprint().to_hex(),
            second_reopened.editor.fold_model_fingerprint_v1()
        );

        let reopened_instance = reopened.instance_id;
        let reopened_project_id = reopened.project_id;
        let reopened_revision = reopened.editor.revision();
        super::super::execute_undo(
            &mut reopened,
            reopened_instance,
            reopened_project_id,
            reopened_revision,
        )
        .expect("undo freshly revalidated stacked fold");
        assert!(reopened.current_layer_evidence.is_none());
        let reopened_redo_revision = reopened.editor.revision();
        super::super::execute_redo(
            &mut reopened,
            reopened_instance,
            reopened_project_id,
            reopened_redo_revision,
        )
        .expect("redo freshly revalidated stacked fold");
        assert!(reopened.current_layer_evidence.is_none());
        assert!(
            reopened
                .project_archive()
                .expect("archive evidence-invalidated reopened project")
                .layer_evidence
                .is_none(),
            "Undo/Redo after fresh revalidation must not resurrect evidence"
        );
}
