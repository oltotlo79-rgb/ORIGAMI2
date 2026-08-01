use super::*;

fn raw_three_transition_path_fixture_v1() -> (
    ori_collision::CertifiedPoseGraphPathCertificateV1,
    ori_domain::FaceId,
    ori_domain::EdgeId,
    String,
) {
    use ori_collision::{
        CertifiedPathGraphSearchResultV1, CertifiedPathTransitionCandidateV1,
        certify_scheduled_cycle_transition_v1, search_certified_pose_graph_v1,
    };
    use ori_core::Command;
    use ori_domain::EdgeKind;
    use ori_kinematics::{
        CanonicalHingeAngles, CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, HingeAngle,
        MaterialHingeGraphAudit, MaterialHingeGraphGeometry, MultiHingePathCandidateLimitsV1,
        TreeKinematicsLimits, generate_linear_multi_hinge_path_candidate_v1,
    };

    let mut project = crate::initial_project_state();
    let edge = ori_domain::EdgeId::new();
    let boundary = project.editor.paper().boundary_vertices.clone();
    project
        .editor
        .execute(
            0,
            Command::AddEdge {
                id: edge,
                start: boundary[0],
                end: boundary[2],
                kind: EdgeKind::Mountain,
            },
        )
        .expect("add one material hinge");
    let model_fingerprint = project.editor.fold_model_fingerprint_v1();
    let topology_analysis = project
        .editor
        .topology_analysis_input(project.project_id)
        .analyze();
    let topology = topology_analysis
        .simulation_snapshot()
        .expect("one-hinge topology");
    let fixed_face = topology.faces[0].id;
    let geometry = MaterialHingeGraphGeometry::prepare(
        project.editor.pattern(),
        project.editor.paper(),
        topology,
        TreeKinematicsLimits::default(),
    )
    .expect("one-hinge graph model");
    let audit = MaterialHingeGraphAudit::prepare(topology, TreeKinematicsLimits::default())
        .expect("one-hinge graph audit");
    let states = [5.0, 20.0, 35.0, 50.0]
        .into_iter()
        .map(|angle_degrees| {
            CanonicalHingeAngles::new(vec![
                HingeAngle::new(edge, angle_degrees).expect("finite fixture angle"),
            ])
            .expect("canonical fixture pose")
        })
        .collect::<Vec<_>>();
    let schedule_limits = CycleScheduleLimitsV1 {
        max_work: 1_048_576,
        ..CycleScheduleLimitsV1::default()
    };
    let mut evidence = Vec::new();
    let mut candidates = Vec::new();
    let mut graph_states = Vec::new();
    for (index, pair) in states.windows(2).enumerate() {
        let [source, target] = pair else {
            unreachable!("windows(2) always has two states")
        };
        let generated = generate_linear_multi_hinge_path_candidate_v1(
            &geometry,
            &audit,
            fixed_face,
            source,
            target,
            MultiHingePathCandidateLimitsV1::default(),
        )
        .expect("one-hinge schedule");
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed_face,
                generated.schedule(),
                1.0e-9,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 8,
                    max_leaves: 256,
                    max_work: 1_048_576,
                    schedule_limits,
                },
            )
            .expect("one-hinge full-domain closure");
        let certified = certify_scheduled_cycle_transition_v1(
            &geometry, &audit, fixed_face, &generated, &closure, 32,
        )
        .expect("native scheduled transition");
        if index == 0 {
            graph_states.push(certified.source());
        }
        graph_states.push(certified.target());
        candidates.push(CertifiedPathTransitionCandidateV1 {
            source: certified.source(),
            target: certified.target(),
            candidate_key: certified.schedule_certificate(),
        });
        evidence.push(certified);
    }
    let source = graph_states[0];
    let target = *graph_states.last().expect("final graph state");
    let path = match search_certified_pose_graph_v1(
        &graph_states,
        &candidates,
        source,
        target,
        |candidate| {
            evidence.iter().copied().find(|value| {
                value.source() == candidate.source
                    && value.target() == candidate.target
                    && value.schedule_certificate() == candidate.candidate_key
            })
        },
    ) {
        CertifiedPathGraphSearchResultV1::Certified(path) => path,
        CertifiedPathGraphSearchResultV1::Indeterminate { .. } => {
            panic!("three certified edges must produce a path")
        }
    };
    assert_eq!(path.edges().len(), 3);
    (path, fixed_face, edge, model_fingerprint)
}

#[test]
fn named_compiler_path_is_instruction_bound_for_whole_segments_and_registry_v1() {
    let (raw, fixed_face, edge, model) = raw_three_transition_path_fixture_v1();
    let source = vec![(edge, 5.0)];
    let targets = vec![vec![(edge, 20.0)], vec![(edge, 35.0)], vec![(edge, 50.0)]];
    assert_eq!(raw.native_source_model_binding_v1(), None);

    let bound =
        issue_instruction_bound_timeline_path_v1(&raw, &model, fixed_face, &source, &targets)
            .expect("live named-compiler path is instruction-bound");
    assert!(bound.is_native_attestable_v1());
    assert_eq!(bound.native_fixed_face_v1(), Some(fixed_face));
    assert!(bound.native_source_model_binding_v1().is_some());
    assert_ne!(bound.binding_fingerprint_v1(), raw.binding_fingerprint_v1());

    let instruction_state = |angle_degrees| {
        vec![InstructionHingeAngle {
            edge,
            angle_degrees,
        }]
    };
    let states = [5.0, 20.0, 35.0, 50.0]
        .into_iter()
        .map(instruction_state)
        .collect::<Vec<_>>();
    for (index, pair) in states.windows(2).enumerate() {
        let segment = bound
            .segment_certificate_v1(index)
            .expect("every named compiler segment is retained");
        assert!(segment.is_native_attestable_v1());
        assert_eq!(segment.native_fixed_face_v1(), Some(fixed_face));
        assert_eq!(
            segment.native_source_model_binding_v1(),
            bound.native_source_model_binding_v1()
        );
        assert_eq!(
            segment.source(),
            ori_instructions::instruction_pose_fingerprint_v1(&model, fixed_face, &pair[0])
        );
        assert_eq!(
            segment.target(),
            ori_instructions::instruction_pose_fingerprint_v1(&model, fixed_face, &pair[1])
        );
    }

    let mut timeline = ori_instructions::append_certified_dyadic_path_timeline_v1(
        &ori_domain::InstructionTimeline::default(),
        "named compiler binding",
        &model,
        fixed_face,
        &states[0],
        &states[1..],
        &bound,
    )
    .expect("compiled timeline and candidate registry use the same bound path");
    for index in 0..bound.edges().len() {
        let segment = bound
            .segment_certificate_v1(index)
            .expect("compiler segment certificate");
        let reference =
            ori_instructions::path_certificate_reference_from_native_v1(&segment, &model)
                .expect("compiler segment reference");
        timeline.steps[index + 1]
            .visual
            .path_certificate_reference_v1 = Some(reference);
    }
    let instance = ori_domain::ProjectId::new();
    let project = ori_domain::ProjectId::new();
    let entries = crate::path_certificate_registry::TrustedPathCertificateRegistryV1::
        prepare_entries_for_timeline_suffix_v1(instance, project, &timeline, 0, Some(&bound))
        .expect("every compiled path reference matches a registry candidate");
    let registry = crate::path_certificate_registry::TrustedPathCertificateRegistryV1::default()
        .with_registered_timeline_v1(instance, project, &timeline, entries)
        .expect("candidate registry accepts the exact compiled timeline");
    assert_eq!(registry.len_v1(), 3);
    assert!(
        ori_instructions::append_certified_dyadic_path_timeline_v1(
            &ori_domain::InstructionTimeline::default(),
            "raw graph path",
            &model,
            fixed_face,
            &states[0],
            &states[1..],
            &raw,
        )
        .is_err(),
        "raw graph-domain authority must not cross the timeline boundary"
    );

    let mut drifted_model = model.clone().into_bytes();
    let last = drifted_model.last_mut().expect("non-empty SHA-256 hex");
    *last = if *last == b'0' { b'1' } else { b'0' };
    let drifted_model = String::from_utf8(drifted_model).expect("ASCII hex");
    assert!(
        issue_instruction_bound_timeline_path_v1(
            &raw,
            &drifted_model,
            fixed_face,
            &source,
            &targets,
        )
        .is_err()
    );
    assert!(
        issue_instruction_bound_timeline_path_v1(
            &raw,
            &model,
            ori_domain::FaceId::new(),
            &source,
            &targets,
        )
        .is_err()
    );
    let mut one_ulp_drift = targets.clone();
    one_ulp_drift[1][0].1 = f64::from_bits(one_ulp_drift[1][0].1.to_bits() + 1);
    assert!(
        issue_instruction_bound_timeline_path_v1(
            &raw,
            &model,
            fixed_face,
            &source,
            &one_ulp_drift,
        )
        .is_err(),
        "one-ULP endpoint drift must fail closed"
    );
    assert!(
        issue_instruction_bound_timeline_path_v1(&raw, &model, fixed_face, &source, &targets[..2],)
            .is_err(),
        "path/timeline arity drift must fail closed"
    );
}
