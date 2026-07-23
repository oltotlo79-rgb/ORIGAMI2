use ori_collision::{
    HingeReliefLinearAngleScheduleV1, HingeReliefPolicyErrorV1, HingeReliefPolicyLimitsV1,
    HingeReliefPolicyRecordV1, certify_hinge_relief_local_intervals_v1,
    prepare_hinge_relief_prerequisite_v1, revalidate_hinge_relief_local_intervals_v1,
    revalidate_hinge_relief_prerequisite_v1,
};
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_kinematics::{MaterialHingeGraphGeometry, TreeKinematicsLimits};
use ori_topology::{FaceExtractionInput, analyze_faces};

fn vertex_id(index: u64) -> VertexId {
    serde_json::from_str(&format!("\"10000000-0000-4000-8000-{index:012x}\"")).unwrap()
}

fn edge_id(index: u64) -> EdgeId {
    serde_json::from_str(&format!("\"10000000-0000-4000-9000-{index:012x}\"")).unwrap()
}

fn project_id(index: u64) -> ProjectId {
    serde_json::from_str(&format!("\"10000000-0000-4000-b000-{index:012x}\"")).unwrap()
}

fn build_graph(
    hinge_count: usize,
    project: ProjectId,
) -> (MaterialHingeGraphGeometry, Vec<EdgeId>) {
    let column_count = hinge_count + 2;
    let mut vertices = Vec::new();
    for index in 0..column_count {
        vertices.push(Vertex {
            id: vertex_id(index as u64 + 1),
            position: Point2::new(index as f64, 0.0),
        });
    }
    for index in (0..column_count).rev() {
        vertices.push(Vertex {
            id: vertex_id(column_count as u64 + (column_count - index) as u64),
            position: Point2::new(index as f64, 1.0),
        });
    }
    let boundary = vertices.iter().map(|vertex| vertex.id).collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: edge_id(index as u64 + 1),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    let mut hinges = Vec::new();
    for index in 1..=hinge_count {
        let id = edge_id(boundary.len() as u64 + index as u64);
        edges.push(Edge {
            id,
            start: vertex_id(index as u64 + 1),
            end: vertex_id((2 * column_count - index) as u64),
            kind: EdgeKind::Mountain,
        });
        hinges.push(id);
    }
    let pattern = CreasePattern { vertices, edges };
    let paper = Paper {
        boundary_vertices: boundary,
        thickness_mm: 0.1,
        ..Paper::default()
    };
    let report = analyze_faces(FaceExtractionInput {
        identity_namespace: project,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    });
    assert!(report.issues.is_empty(), "{:?}", report.issues);
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &report.snapshot.unwrap(),
        TreeKinematicsLimits::default(),
    )
    .unwrap();
    hinges.sort_unstable_by_key(EdgeId::canonical_bytes);
    (geometry, hinges)
}

fn records(edges: &[EdgeId]) -> Vec<HingeReliefPolicyRecordV1> {
    edges
        .iter()
        .map(|&edge| HingeReliefPolicyRecordV1 {
            edge,
            cutout_width_mm: 0.1,
            bevel_angle_degrees: 90.0,
            material_thickness_mm: 0.1,
        })
        .collect()
}

fn schedules(edges: &[EdgeId]) -> Vec<HingeReliefLinearAngleScheduleV1> {
    edges
        .iter()
        .map(|&edge| HingeReliefLinearAngleScheduleV1 {
            edge,
            source_angle_degrees: 90.0,
            target_angle_degrees: 120.0,
        })
        .collect()
}

#[test]
fn actual_material_graph_binding_is_complete_at_four_eight_sixteen() {
    for count in [4, 8, 16] {
        let (graph, edges) = build_graph(count, project_id(count as u64));
        let records = records(&edges);
        let proof = prepare_hinge_relief_prerequisite_v1(
            &graph,
            0.1,
            &records,
            HingeReliefPolicyLimitsV1::default(),
        )
        .unwrap();
        revalidate_hinge_relief_prerequisite_v1(
            &proof,
            &graph,
            0.1,
            &records,
            HingeReliefPolicyLimitsV1::default(),
        )
        .unwrap();
        let schedules = schedules(&edges);
        let certificate = certify_hinge_relief_local_intervals_v1(
            &proof,
            &graph,
            0.1,
            &records,
            &schedules,
            HingeReliefPolicyLimitsV1::default(),
        )
        .unwrap();
        assert!(!certificate.authorizes_whole_path());
        assert!(!certificate.authorizes_project_mutation());
        assert!(!certificate.authorizes_shared_hinge_admission());
        assert_eq!(certificate.schedule_count(), count);
        revalidate_hinge_relief_local_intervals_v1(
            &certificate,
            &proof,
            &graph,
            0.1,
            &records,
            &schedules,
            HingeReliefPolicyLimitsV1::default(),
        )
        .unwrap();

        let mut tampered = schedules.clone();
        tampered[0].source_angle_degrees = 91.0;
        assert_eq!(
            revalidate_hinge_relief_local_intervals_v1(
                &certificate,
                &proof,
                &graph,
                0.1,
                &records,
                &tampered,
                HingeReliefPolicyLimitsV1::default(),
            ),
            Err(HingeReliefPolicyErrorV1::BindingMismatch)
        );
        assert!(matches!(
            certify_hinge_relief_local_intervals_v1(
                &proof,
                &graph,
                0.1,
                &records,
                &schedules[..schedules.len() - 1],
                HingeReliefPolicyLimitsV1::default(),
            ),
            Err(HingeReliefPolicyErrorV1::ScheduleBindingMismatch)
        ));
    }
}

#[test]
fn graph_record_thickness_and_unknown_hinge_tamper_fail_closed() {
    let (graph, edges) = build_graph(4, project_id(20));
    let records = records(&edges);
    let proof = prepare_hinge_relief_prerequisite_v1(
        &graph,
        0.1,
        &records,
        HingeReliefPolicyLimitsV1::default(),
    )
    .unwrap();
    let (foreign, _) = build_graph(4, project_id(21));
    assert_eq!(
        revalidate_hinge_relief_prerequisite_v1(
            &proof,
            &foreign,
            0.1,
            &records,
            HingeReliefPolicyLimitsV1::default(),
        ),
        Err(HingeReliefPolicyErrorV1::BindingMismatch)
    );
    assert_eq!(
        revalidate_hinge_relief_prerequisite_v1(
            &proof,
            &graph,
            0.2,
            &records,
            HingeReliefPolicyLimitsV1::default(),
        ),
        Err(HingeReliefPolicyErrorV1::ThicknessMismatch)
    );
    let mut unknown = records.clone();
    unknown[0].edge = EdgeId::new();
    unknown.sort_unstable_by_key(|record| record.edge.canonical_bytes());
    assert_eq!(
        revalidate_hinge_relief_prerequisite_v1(
            &proof,
            &graph,
            0.1,
            &unknown,
            HingeReliefPolicyLimitsV1::default(),
        ),
        Err(HingeReliefPolicyErrorV1::UnknownHinge)
    );

    let mut changed = records.clone();
    changed[0].cutout_width_mm = 0.2;
    assert_eq!(
        revalidate_hinge_relief_prerequisite_v1(
            &proof,
            &graph,
            0.1,
            &changed,
            HingeReliefPolicyLimitsV1::default(),
        ),
        Err(HingeReliefPolicyErrorV1::BindingMismatch)
    );
    changed.clone_from(&records);
    changed[0].bevel_angle_degrees = 120.0;
    assert_eq!(
        revalidate_hinge_relief_prerequisite_v1(
            &proof,
            &graph,
            0.1,
            &changed,
            HingeReliefPolicyLimitsV1::default(),
        ),
        Err(HingeReliefPolicyErrorV1::BindingMismatch)
    );
}

#[test]
fn graph_hinge_cap_is_checked_before_policy_allocation() {
    let (graph, _) = build_graph(257, project_id(30));
    assert!(matches!(
        prepare_hinge_relief_prerequisite_v1(
            &graph,
            0.1,
            &[],
            HingeReliefPolicyLimitsV1::default(),
        ),
        Err(HingeReliefPolicyErrorV1::ResourceLimit)
    ));
}
