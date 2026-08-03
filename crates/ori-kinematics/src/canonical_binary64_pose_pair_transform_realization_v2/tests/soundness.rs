use super::*;

#[test]
fn single_face_zero_hinge_pair_is_a_canonical_identity_realization_v2() {
    let namespace = ProjectId::schema_namespace([0x19; 16]);
    let face = FaceId::derive_v5(namespace, b"only-face");
    let topology = TopologySnapshot {
        source_revision: 1,
        faces: vec![Face {
            id: face,
            key: FaceKey(face.canonical_bytes().repeat(2).try_into().unwrap()),
            outer: BoundaryWalk {
                half_edges: Vec::new(),
                signed_double_area: 1.0,
            },
            holes: Vec::new(),
            seams: Vec::new(),
            area: 0.5,
        }],
        edge_incidence: Vec::new(),
        hinge_adjacency: Vec::new(),
        material_components: Vec::new(),
    };
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let geometry = MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), Vec::new());
    let angles = CanonicalHingeAngles::new(Vec::new()).unwrap();
    let lower_pose = geometry
        .solve_closed(&audit, face, &angles, OBSERVATION_TOLERANCE_V2)
        .unwrap();
    let upper_pose = geometry
        .solve_closed(&audit, face, &angles, OBSERVATION_TOLERANCE_V2)
        .unwrap();
    let bound = geometry
        .checked_canonical_binary64_pose_pair_transform_realization_resource_bound_v2(
            &audit,
            &lower_pose,
            &upper_pose,
        )
        .unwrap();
    let input = CanonicalBinary64PosePairTransformRealizationInputV2 {
        geometry: &geometry,
        audit: &audit,
        fixed_face: face,
        lower_pose: &lower_pose,
        upper_pose: &upper_pose,
        limits: limits_with_slack_v2(bound),
    };
    let evidence =
        prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(input).unwrap();
    assert_eq!(evidence.face_count_v2(), 1);
    assert_eq!(evidence.hinge_count_v2(), 0);
    assert!(
        evidence.workspace_structural_requirement_bytes_v2()
            <= evidence.workspace_peak_bytes_upper_bound_v2()
    );
    evidence.revalidate_v2(input).unwrap();
}

#[test]
fn closure_hinge_is_partitioned_without_entering_transform_propagation_v2() {
    let namespace = ProjectId::schema_namespace([0x2a; 16]);
    let faces = (0_u8..3)
        .map(|index| FaceId::derive_v5(namespace, &[b'f', index]))
        .collect::<Vec<_>>();
    let edges = (0_u8..3)
        .map(|index| EdgeId::derive_v5(namespace, &[b'e', index]))
        .collect::<Vec<_>>();
    let pairs = [(0, 1), (1, 2), (2, 0)];
    let topology = TopologySnapshot {
        source_revision: 1,
        faces: faces
            .iter()
            .map(|id| Face {
                id: *id,
                key: FaceKey(id.canonical_bytes().repeat(2).try_into().unwrap()),
                outer: BoundaryWalk {
                    half_edges: Vec::new(),
                    signed_double_area: 1.0,
                },
                holes: Vec::new(),
                seams: Vec::new(),
                area: 0.5,
            })
            .collect(),
        edge_incidence: Vec::new(),
        hinge_adjacency: edges
            .iter()
            .zip(pairs)
            .map(|(edge, (left, right))| FaceAdjacency {
                edge: *edge,
                first: faces[left],
                second: faces[right],
                assignment: FoldAssignment::Mountain,
            })
            .collect(),
        material_components: Vec::new(),
    };
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    assert_eq!(audit.spanning_hinges().len(), 2);
    assert_eq!(audit.closure_hinges().len(), 1);
    let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
    let axis = Point3::new(1.0, 0.0, 0.0).unwrap();
    let mut hinges = edges
        .iter()
        .zip(pairs)
        .map(|(edge, (left, right))| {
            TreeHinge::new_for_test(
                *edge,
                FoldAssignment::Mountain,
                faces[left],
                faces[right],
                origin,
                axis,
                axis,
            )
        })
        .collect::<Vec<_>>();
    hinges.sort_unstable_by_key(|hinge| hinge.edge().canonical_bytes());
    let geometry = MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), hinges);
    let angles = CanonicalHingeAngles::new(
        geometry
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
            .collect(),
    )
    .unwrap();
    let lower_pose = geometry
        .solve_closed(&audit, faces[0], &angles, OBSERVATION_TOLERANCE_V2)
        .unwrap();
    let upper_pose = geometry
        .solve_closed(&audit, faces[0], &angles, OBSERVATION_TOLERANCE_V2)
        .unwrap();
    let bound = geometry
        .checked_canonical_binary64_pose_pair_transform_realization_resource_bound_v2(
            &audit,
            &lower_pose,
            &upper_pose,
        )
        .unwrap();
    let input = CanonicalBinary64PosePairTransformRealizationInputV2 {
        geometry: &geometry,
        audit: &audit,
        fixed_face: faces[0],
        lower_pose: &lower_pose,
        upper_pose: &upper_pose,
        limits: limits_with_slack_v2(bound),
    };
    let evidence =
        prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(input).unwrap();
    assert_eq!(evidence.hinge_count_v2(), 3);
    evidence.revalidate_v2(input).unwrap();

    let mut sorted_edges = edges.clone();
    sorted_edges.sort_unstable_by_key(EdgeId::canonical_bytes);
    let alternate_topology = TopologySnapshot {
        source_revision: 2,
        faces: topology.faces.clone(),
        edge_incidence: Vec::new(),
        hinge_adjacency: vec![
            FaceAdjacency {
                edge: sorted_edges[0],
                first: faces[0],
                second: faces[1],
                assignment: FoldAssignment::Mountain,
            },
            FaceAdjacency {
                edge: sorted_edges[1],
                first: faces[0],
                second: faces[1],
                assignment: FoldAssignment::Mountain,
            },
            FaceAdjacency {
                edge: sorted_edges[2],
                first: faces[1],
                second: faces[2],
                assignment: FoldAssignment::Mountain,
            },
        ],
        material_components: Vec::new(),
    };
    let alternate_audit =
        MaterialHingeGraphAudit::prepare(&alternate_topology, TreeKinematicsLimits::default())
            .unwrap();
    assert_ne!(alternate_audit, audit);
    assert_eq!(
        evidence.revalidate_v2(CanonicalBinary64PosePairTransformRealizationInputV2 {
            audit: &alternate_audit,
            ..input
        }),
        Err(CanonicalBinary64PosePairTransformRealizationErrorV2::CertificateBindingMismatch)
    );
}

#[test]
fn canonical_pair_issues_replays_and_grants_only_the_narrow_fact_v2() {
    let fixture = fixture_v2();
    let evidence =
        prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(fixture.input_v2())
            .unwrap();
    assert_eq!(evidence.realized_pose_count_v2(), 2);
    assert_eq!(evidence.face_count_v2(), fixture.geometry.face_ids().len());
    assert_eq!(evidence.hinge_count_v2(), fixture.geometry.hinges().len());
    assert_eq!(evidence.fixed_face_v2(), fixture.fixed_face);
    assert!(evidence.matches_geometry_instance_v2(&fixture.geometry));
    assert!(evidence.matches_pose_instances_v2(&fixture.lower_pose, &fixture.upper_pose));
    assert!(evidence.proves_both_pose_instances_are_canonical_binary64_transform_realizations_v2());
    assert!(!evidence.authorizes_source_target_identity());
    assert!(!evidence.authorizes_current_requested_identity());
    assert!(!evidence.authorizes_application_parameter_identity());
    assert!(!evidence.authorizes_direction());
    assert!(!evidence.authorizes_layer_order());
    assert!(!evidence.authorizes_exact_closure());
    assert!(!evidence.authorizes_transform_realization());
    assert!(!evidence.authorizes_pose_realization());
    assert!(!evidence.authorizes_continuous_motion());
    assert!(!evidence.authorizes_collision_clearance());
    assert!(!evidence.authorizes_layer_transport());
    assert!(!evidence.authorizes_project_mutation());
    assert!(!evidence.authorizes_apply());
    assert!(!evidence.authorizes_viewer());
    assert!(!evidence.authorizes_export());
    assert_ne!(evidence.binding_fingerprint_v2(), [0; 32]);
    assert!(evidence.replay_policy_matches_v2(fixture.limits));
    evidence.revalidate_v2(fixture.input_v2()).unwrap();
}

#[test]
fn same_values_with_fresh_or_swapped_pose_arcs_fail_closed_v2() {
    let fixture = fixture_v2();
    let evidence =
        prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(fixture.input_v2())
            .unwrap();
    let fresh_lower = fixture.fresh_pose_v2(true);
    let fresh_limits = limits_for_pose_pair_v2(&fixture, &fresh_lower, &fixture.upper_pose);
    assert_eq!(fresh_limits, fixture.limits);
    let fresh_input = CanonicalBinary64PosePairTransformRealizationInputV2 {
        lower_pose: &fresh_lower,
        ..fixture.input_v2()
    };
    assert_eq!(
        evidence.revalidate_v2(fresh_input),
        Err(CanonicalBinary64PosePairTransformRealizationErrorV2::CertificateBindingMismatch)
    );
    let swapped = CanonicalBinary64PosePairTransformRealizationInputV2 {
        lower_pose: &fixture.upper_pose,
        upper_pose: &fixture.lower_pose,
        ..fixture.input_v2()
    };
    assert_eq!(
        evidence.revalidate_v2(swapped),
        Err(CanonicalBinary64PosePairTransformRealizationErrorV2::CertificateBindingMismatch)
    );
    let different_fixed_face = *fixture
        .geometry
        .face_ids()
        .iter()
        .find(|face| **face != fixture.fixed_face)
        .unwrap();
    assert_eq!(
        evidence.revalidate_v2(CanonicalBinary64PosePairTransformRealizationInputV2 {
            fixed_face: different_fixed_face,
            ..fixture.input_v2()
        }),
        Err(CanonicalBinary64PosePairTransformRealizationErrorV2::CertificateBindingMismatch)
    );
}

#[test]
fn same_shape_geometry_aba_fails_closed_v2() {
    let fixture = fixture_v2();
    let evidence =
        prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(fixture.input_v2())
            .unwrap();
    let aba_geometry = MaterialHingeGraphGeometry::new_for_test(
        fixture.geometry.face_ids().to_vec(),
        fixture.geometry.hinges().to_vec(),
    );
    let aba_lower = aba_geometry
        .solve_closed(
            &fixture.audit,
            fixture.fixed_face,
            fixture.lower_pose.hinge_angles(),
            OBSERVATION_TOLERANCE_V2,
        )
        .unwrap();
    let aba_upper = aba_geometry
        .solve_closed(
            &fixture.audit,
            fixture.fixed_face,
            fixture.upper_pose.hinge_angles(),
            OBSERVATION_TOLERANCE_V2,
        )
        .unwrap();
    let input = CanonicalBinary64PosePairTransformRealizationInputV2 {
        geometry: &aba_geometry,
        audit: &fixture.audit,
        fixed_face: fixture.fixed_face,
        lower_pose: &aba_lower,
        upper_pose: &aba_upper,
        limits: limits_with_slack_v2(
            aba_geometry
                .checked_canonical_binary64_pose_pair_transform_realization_resource_bound_v2(
                    &fixture.audit,
                    &aba_lower,
                    &aba_upper,
                )
                .unwrap(),
        ),
    };
    assert_eq!(input.limits, fixture.limits);
    assert_eq!(
        evidence.revalidate_v2(input),
        Err(CanonicalBinary64PosePairTransformRealizationErrorV2::CertificateBindingMismatch)
    );
}

#[test]
fn tolerance_accepted_global_frame_drift_is_not_canonical_realization_v2() {
    let fixture = fixture_v2();
    let drifted = globally_drifted_pose_v2(&fixture, &fixture.lower_pose);
    let input = CanonicalBinary64PosePairTransformRealizationInputV2 {
        lower_pose: &drifted,
        limits: limits_for_pose_pair_v2(&fixture, &drifted, &fixture.upper_pose),
        ..fixture.input_v2()
    };
    assert!(matches!(
        prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(input),
        Err(CanonicalBinary64PosePairTransformRealizationErrorV2::TransformMismatch)
    ));
}

#[test]
fn tolerance_accepted_face_transform_or_angle_drift_fails_v2() {
    let fixture = fixture_v2();
    let mut face_candidate = fixture.lower_pose.transforms().to_vec();
    let drifting_index = usize::from(face_candidate[0].face() == fixture.fixed_face);
    face_candidate[drifting_index] = CandidateFaceTransform::new(
        face_candidate[drifting_index].face(),
        RigidTransform::identity(),
    );
    let face_drift = fixture
        .geometry
        .observe_closed(
            &fixture.audit,
            fixture.fixed_face,
            fixture.lower_pose.hinge_angles(),
            &face_candidate,
            f64::MAX,
        )
        .unwrap();
    let face_input = CanonicalBinary64PosePairTransformRealizationInputV2 {
        lower_pose: &face_drift,
        limits: limits_for_pose_pair_v2(&fixture, &face_drift, &fixture.upper_pose),
        ..fixture.input_v2()
    };
    assert!(matches!(
        prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(face_input),
        Err(CanonicalBinary64PosePairTransformRealizationErrorV2::TransformMismatch)
    ));

    let mut angle_values = fixture.lower_pose.hinge_angles().as_slice().to_vec();
    angle_values[0] = HingeAngle::new(
        angle_values[0].edge(),
        f64::from_bits(angle_values[0].angle_degrees().to_bits() + 1),
    )
    .unwrap();
    let angle_values = CanonicalHingeAngles::new(angle_values).unwrap();
    let angle_drift = fixture
        .geometry
        .observe_closed(
            &fixture.audit,
            fixture.fixed_face,
            &angle_values,
            fixture.lower_pose.transforms(),
            f64::MAX,
        )
        .unwrap();
    let angle_input = CanonicalBinary64PosePairTransformRealizationInputV2 {
        lower_pose: &angle_drift,
        limits: limits_for_pose_pair_v2(&fixture, &angle_drift, &fixture.upper_pose),
        ..fixture.input_v2()
    };
    assert!(matches!(
        prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(angle_input),
        Err(CanonicalBinary64PosePairTransformRealizationErrorV2::TransformMismatch)
    ));
}

#[test]
fn debug_is_redacted_v2() {
    let fixture = fixture_v2();
    let evidence =
        prove_canonical_binary64_pose_pair_transform_realization_evidence_v2(fixture.input_v2())
            .unwrap();
    let debug = format!("{evidence:?}");
    assert!(debug.contains("CanonicalBinary64PosePairTransformRealizationEvidenceV2"));
    for secret in [
        "binding_fingerprint",
        "lower_pose_instance",
        "upper_pose_instance",
        "audit_binding",
    ] {
        assert!(!debug.contains(secret));
    }
}
