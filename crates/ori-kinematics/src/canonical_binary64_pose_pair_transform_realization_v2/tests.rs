use ori_domain::{EdgeId, FaceId, ProjectId};
use ori_topology::{BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot};

use crate::{
    CandidateFaceTransform, CanonicalHingeAngles, HingeAngle, Point3, RigidTransform, TreeHinge,
    TreeKinematicsLimits,
};

use super::*;

#[path = "tests/policy_and_stop.rs"]
mod policy_and_stop;
#[path = "tests/soundness.rs"]
mod soundness;

const OBSERVATION_TOLERANCE_V2: f64 = 1.0e-9;

struct FixtureV2 {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    fixed_face: FaceId,
    lower_pose: ClosedMaterialHingeGraphPose,
    upper_pose: ClosedMaterialHingeGraphPose,
    limits: CanonicalBinary64PosePairTransformRealizationLimitsV2,
}

impl FixtureV2 {
    fn input_v2(&self) -> CanonicalBinary64PosePairTransformRealizationInputV2<'_> {
        CanonicalBinary64PosePairTransformRealizationInputV2 {
            geometry: &self.geometry,
            audit: &self.audit,
            fixed_face: self.fixed_face,
            lower_pose: &self.lower_pose,
            upper_pose: &self.upper_pose,
            limits: self.limits,
        }
    }

    fn fresh_pose_v2(&self, lower: bool) -> ClosedMaterialHingeGraphPose {
        let source = if lower {
            &self.lower_pose
        } else {
            &self.upper_pose
        };
        self.geometry
            .solve_closed(
                &self.audit,
                self.fixed_face,
                source.hinge_angles(),
                OBSERVATION_TOLERANCE_V2,
            )
            .unwrap()
    }
}

fn fixture_v2() -> FixtureV2 {
    let namespace = ProjectId::schema_namespace([0x6d; 16]);
    let raw_faces =
        [&b"root"[..], &b"middle"[..], &b"leaf"[..]].map(|name| FaceId::derive_v5(namespace, name));
    let raw_edges = [&b"first"[..], &b"second"[..]].map(|name| EdgeId::derive_v5(namespace, name));
    let topology = TopologySnapshot {
        source_revision: 1,
        faces: raw_faces
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
        hinge_adjacency: vec![
            FaceAdjacency {
                edge: raw_edges[0],
                first: raw_faces[0],
                second: raw_faces[1],
                assignment: FoldAssignment::Mountain,
            },
            FaceAdjacency {
                edge: raw_edges[1],
                first: raw_faces[1],
                second: raw_faces[2],
                assignment: FoldAssignment::Valley,
            },
        ],
        material_components: Vec::new(),
    };
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let x_axis = Point3::new(1.0, 0.0, 0.0).unwrap();
    let y_axis = Point3::new(0.0, 1.0, 0.0).unwrap();
    let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
    let mut hinges = vec![
        TreeHinge::new_for_test(
            raw_edges[0],
            FoldAssignment::Mountain,
            raw_faces[0],
            raw_faces[1],
            origin,
            x_axis,
            x_axis,
        ),
        TreeHinge::new_for_test(
            raw_edges[1],
            FoldAssignment::Valley,
            raw_faces[1],
            raw_faces[2],
            origin,
            y_axis,
            y_axis,
        ),
    ];
    hinges.sort_unstable_by_key(|hinge| hinge.edge().canonical_bytes());
    let geometry = MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), hinges);
    let lower_angles = angles_v2(&geometry, [15.0, 20.0]);
    let upper_angles = angles_v2(&geometry, [35.0, 10.0]);
    let fixed_face = raw_faces[0];
    let lower_pose = geometry
        .solve_closed(&audit, fixed_face, &lower_angles, OBSERVATION_TOLERANCE_V2)
        .unwrap();
    let upper_pose = geometry
        .solve_closed(&audit, fixed_face, &upper_angles, OBSERVATION_TOLERANCE_V2)
        .unwrap();
    let bound = geometry
        .checked_canonical_binary64_pose_pair_transform_realization_resource_bound_v2(
            &audit,
            &lower_pose,
            &upper_pose,
        )
        .unwrap();
    let limits = limits_with_slack_v2(bound);
    FixtureV2 {
        geometry,
        audit,
        fixed_face,
        lower_pose,
        upper_pose,
        limits,
    }
}

fn branching_fixture_v2(face_count: usize) -> FixtureV2 {
    assert!(face_count >= 2);
    let namespace = ProjectId::schema_namespace([0x7b; 16]);
    let raw_faces = (0..face_count)
        .map(|index| FaceId::derive_v5(namespace, &index.to_le_bytes()))
        .collect::<Vec<_>>();
    let raw_edges = (1..face_count)
        .map(|index| {
            let mut name = b"edge".to_vec();
            name.extend_from_slice(&index.to_le_bytes());
            EdgeId::derive_v5(namespace, &name)
        })
        .collect::<Vec<_>>();
    let topology = TopologySnapshot {
        source_revision: 1,
        faces: raw_faces
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
        hinge_adjacency: raw_edges
            .iter()
            .zip(raw_faces.iter().skip(1))
            .map(|(edge, leaf)| FaceAdjacency {
                edge: *edge,
                first: raw_faces[0],
                second: *leaf,
                assignment: FoldAssignment::Mountain,
            })
            .collect(),
        material_components: Vec::new(),
    };
    let audit =
        MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
    let origin = Point3::new(0.0, 0.0, 0.0).unwrap();
    let axis = Point3::new(1.0, 0.0, 0.0).unwrap();
    let mut hinges = raw_edges
        .iter()
        .zip(raw_faces.iter().skip(1))
        .map(|(edge, leaf)| {
            TreeHinge::new_for_test(
                *edge,
                FoldAssignment::Mountain,
                raw_faces[0],
                *leaf,
                origin,
                axis,
                axis,
            )
        })
        .collect::<Vec<_>>();
    hinges.sort_unstable_by_key(|hinge| hinge.edge().canonical_bytes());
    let geometry = MaterialHingeGraphGeometry::new_for_test(audit.faces().to_vec(), hinges);
    let make_angles = |offset: f64| {
        CanonicalHingeAngles::new(
            geometry
                .hinges()
                .iter()
                .enumerate()
                .map(|(index, hinge)| {
                    HingeAngle::new(hinge.edge(), offset + (index % 17) as f64).unwrap()
                })
                .collect(),
        )
        .unwrap()
    };
    let fixed_face = raw_faces[0];
    let lower_pose = geometry
        .solve_closed(
            &audit,
            fixed_face,
            &make_angles(5.0),
            OBSERVATION_TOLERANCE_V2,
        )
        .unwrap();
    let upper_pose = geometry
        .solve_closed(
            &audit,
            fixed_face,
            &make_angles(25.0),
            OBSERVATION_TOLERANCE_V2,
        )
        .unwrap();
    let bound = geometry
        .checked_canonical_binary64_pose_pair_transform_realization_resource_bound_v2(
            &audit,
            &lower_pose,
            &upper_pose,
        )
        .unwrap();
    FixtureV2 {
        geometry,
        audit,
        fixed_face,
        lower_pose,
        upper_pose,
        limits: limits_with_slack_v2(bound),
    }
}

fn angles_v2(geometry: &MaterialHingeGraphGeometry, values: [f64; 2]) -> CanonicalHingeAngles {
    CanonicalHingeAngles::new(
        geometry
            .hinges()
            .iter()
            .zip(values)
            .map(|(hinge, value)| HingeAngle::new(hinge.edge(), value).unwrap())
            .collect(),
    )
    .unwrap()
}

fn limits_with_slack_v2(
    bound: CanonicalBinary64PosePairTransformRealizationResourceBoundV2,
) -> CanonicalBinary64PosePairTransformRealizationLimitsV2 {
    CanonicalBinary64PosePairTransformRealizationLimitsV2 {
        max_faces: bound.face_count_v2() + 1,
        max_hinges: bound.hinge_count_v2() + 1,
        max_pose_pair_deep_retained_bytes: bound.pose_pair_deep_retained_bytes_v2() + 1,
        max_logical_work: bound.logical_work_required_v2(),
        max_workspace_bytes: bound.workspace_structural_requirement_bytes_v2() + 1_024,
    }
}

fn limits_for_pose_pair_v2(
    fixture: &FixtureV2,
    lower_pose: &ClosedMaterialHingeGraphPose,
    upper_pose: &ClosedMaterialHingeGraphPose,
) -> CanonicalBinary64PosePairTransformRealizationLimitsV2 {
    let bound = fixture
        .geometry
        .checked_canonical_binary64_pose_pair_transform_realization_resource_bound_v2(
            &fixture.audit,
            lower_pose,
            upper_pose,
        )
        .unwrap();
    limits_with_slack_v2(bound)
}

fn globally_drifted_pose_v2(
    fixture: &FixtureV2,
    source: &ClosedMaterialHingeGraphPose,
) -> ClosedMaterialHingeGraphPose {
    let global = RigidTransform::around_axis(
        Point3::new(0.5, -0.25, 0.75).unwrap(),
        Point3::new(0.0, 0.0, 1.0).unwrap(),
        27.0,
    )
    .unwrap();
    let candidate = source
        .transforms()
        .iter()
        .map(|transform| {
            CandidateFaceTransform::new(
                transform.face(),
                global.compose(transform.transform()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    fixture
        .geometry
        .observe_closed(
            &fixture.audit,
            fixture.fixed_face,
            source.hinge_angles(),
            &candidate,
            1.0e-6,
        )
        .unwrap()
}
