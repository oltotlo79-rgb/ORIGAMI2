use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex};
use ori_kinematics::{
    MaterialHingeGraphGeometry, MaterialTreeKinematicsModel, TreeKinematicsLimits,
    prepare_effective_cut_kinematics_diagnostic_v1,
};
use ori_topology::{
    FaceExtractionInput, diagnose_cut_material_component_selection_v1,
    diagnose_effective_cut_material_snapshot_v1,
};
use serde::de::DeserializeOwned;

fn id<T: DeserializeOwned>(suffix: u64) -> T {
    serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\"")).unwrap()
}

fn vertex(suffix: u64, x: f64, y: f64) -> Vertex {
    Vertex {
        id: id(suffix),
        position: Point2::new(x, y),
    }
}

fn edge(suffix: u64, a: &Vertex, b: &Vertex, kind: EdgeKind) -> Edge {
    Edge {
        id: id(suffix),
        start: a.id,
        end: b.id,
        kind,
    }
}

fn fixture(radial_hinges: bool) -> (ProjectId, Paper, CreasePattern) {
    let a = vertex(1, 0.0, 0.0);
    let b = vertex(2, 12.0, 0.0);
    let c = vertex(3, 12.0, 8.0);
    let d = vertex(4, 0.0, 8.0);
    let p = vertex(5, 2.0, 2.0);
    let q = vertex(6, 5.0, 2.0);
    let r = vertex(7, 3.5, 5.0);
    let vertices = vec![
        a.clone(),
        b.clone(),
        c.clone(),
        d.clone(),
        p.clone(),
        q.clone(),
        r.clone(),
    ];
    let mut edges = vec![
        edge(20, &a, &b, EdgeKind::Boundary),
        edge(21, &b, &c, EdgeKind::Boundary),
        edge(22, &c, &d, EdgeKind::Boundary),
        edge(23, &d, &a, EdgeKind::Boundary),
        edge(30, &p, &q, EdgeKind::Cut),
        edge(31, &q, &r, EdgeKind::Cut),
        edge(32, &r, &p, EdgeKind::Cut),
    ];
    if radial_hinges {
        edges.extend([
            edge(40, &p, &a, EdgeKind::Mountain),
            edge(41, &q, &b, EdgeKind::Valley),
        ]);
    }
    let paper = Paper {
        boundary_vertices: vec![a.id, b.id, c.id, d.id],
        cutting_allowed: true,
        ..Paper::default()
    };
    (id(100), paper, CreasePattern { vertices, edges })
}

fn input<'a>(
    namespace: ProjectId,
    revision: u64,
    paper: &'a Paper,
    pattern: &'a CreasePattern,
) -> FaceExtractionInput<'a> {
    FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: revision,
        paper,
        pattern,
    }
}

fn effective<'a>(
    source: FaceExtractionInput<'a>,
) -> ori_topology::EffectiveCutMaterialSnapshotDiagnosticV1 {
    let selection =
        diagnose_cut_material_component_selection_v1(source, Default::default()).unwrap();
    let candidates = selection
        .selections()
        .iter()
        .filter(|entry| !entry.owns_original_boundary)
        .map(|entry| entry.component)
        .collect::<Vec<_>>();
    diagnose_effective_cut_material_snapshot_v1(source, &candidates, Default::default()).unwrap()
}

#[test]
fn isolated_hole_fails_closed_without_silently_filling_absent_material() {
    let (namespace, paper, pattern) = fixture(false);
    let source = input(namespace, 7, &paper, &pattern);
    let token = effective(source);
    assert!(
        prepare_effective_cut_kinematics_diagnostic_v1(&token, source, Default::default()).is_err()
    );
}

#[test]
fn retained_radial_hinges_are_counted_but_raw_cut_prepare_still_fails() {
    let (namespace, paper, pattern) = fixture(true);
    let source = input(namespace, 9, &paper, &pattern);
    let token = effective(source);
    let diagnostic =
        prepare_effective_cut_kinematics_diagnostic_v1(&token, source, Default::default()).unwrap();
    assert_eq!(diagnostic.face_count(), 2);
    assert_eq!(diagnostic.hinge_count(), 2);
    assert!(!diagnostic.authorizes_simulation_admission());
    assert!(!diagnostic.authorizes_pose_solving());
    assert!(!diagnostic.authorizes_project_mutation());
    assert!(!diagnostic.authorizes_persistence());
    assert!(diagnostic.is_for(&token, source, Default::default()));
    assert!(
        MaterialHingeGraphGeometry::prepare(&pattern, &paper, token.snapshot(), Default::default())
            .is_err()
    );
    assert!(
        MaterialTreeKinematicsModel::prepare(
            &pattern,
            &paper,
            token.snapshot(),
            Default::default()
        )
        .is_err()
    );
}

#[test]
fn retained_disjoint_cut_sibling_fails_closed() {
    let (namespace, paper, mut pattern) = fixture(false);
    let s = vertex(8, 7.0, 2.0);
    let t = vertex(9, 10.0, 2.0);
    let u = vertex(10, 8.5, 5.0);
    pattern.vertices.extend([s.clone(), t.clone(), u.clone()]);
    pattern.edges.extend([
        edge(33, &s, &t, EdgeKind::Cut),
        edge(34, &t, &u, EdgeKind::Cut),
        edge(35, &u, &s, EdgeKind::Cut),
    ]);
    let source = input(namespace, 11, &paper, &pattern);
    let selection =
        diagnose_cut_material_component_selection_v1(source, Default::default()).unwrap();
    let candidates = selection
        .selections()
        .iter()
        .filter(|entry| !entry.owns_original_boundary)
        .map(|entry| entry.component)
        .collect::<Vec<_>>();
    let token =
        diagnose_effective_cut_material_snapshot_v1(source, &candidates[..1], Default::default())
            .unwrap();
    assert!(
        prepare_effective_cut_kinematics_diagnostic_v1(&token, source, Default::default()).is_err()
    );
}

#[test]
fn foreign_revision_geometry_paper_and_caps_fail_closed() {
    let (namespace, paper, pattern) = fixture(false);
    let source = input(namespace, 7, &paper, &pattern);
    let token = effective(source);
    assert!(
        prepare_effective_cut_kinematics_diagnostic_v1(
            &token,
            input(namespace, 8, &paper, &pattern),
            Default::default(),
        )
        .is_err()
    );
    let mut changed = pattern.clone();
    changed.vertices[4].position.x += 0.125;
    assert!(
        prepare_effective_cut_kinematics_diagnostic_v1(
            &token,
            input(namespace, 7, &paper, &changed),
            Default::default(),
        )
        .is_err()
    );
    let mut changed_paper = paper.clone();
    changed_paper.thickness_mm += 0.1;
    assert!(
        prepare_effective_cut_kinematics_diagnostic_v1(
            &token,
            input(namespace, 7, &changed_paper, &pattern),
            Default::default(),
        )
        .is_err()
    );
    assert!(
        prepare_effective_cut_kinematics_diagnostic_v1(
            &token,
            source,
            TreeKinematicsLimits {
                max_source_edges: 1,
                ..Default::default()
            },
        )
        .is_err()
    );
}
