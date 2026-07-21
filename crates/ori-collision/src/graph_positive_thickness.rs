use num_rational::BigRational;
use num_traits::Signed;
use ori_kinematics::{ClosedMaterialHingeGraphPose, MaterialHingeGraphGeometry, Point3};
use std::{collections::HashSet, sync::Arc};

pub const POSITIVE_THICKNESS_GRAPH_GEOMETRY_PROOF_V1: &str =
    "positive_thickness_graph_geometry_proof_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositiveThicknessGraphLimitsV1 {
    pub max_unordered_face_pairs: usize,
    pub max_shared_feature_pairs: usize,
}

impl Default for PositiveThicknessGraphLimitsV1 {
    fn default() -> Self {
        Self {
            max_unordered_face_pairs: 1_176,
            max_shared_feature_pairs: 1_176,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositiveThicknessGraphProofErrorV1 {
    InvalidInput,
    ResourceLimit,
    PairEvidenceUnavailable,
}

#[derive(Debug, Clone)]
pub struct NativePositiveThicknessGraphGeometryProofV1 {
    identity: Arc<()>,
    geometry: MaterialHingeGraphGeometry,
    pose: ClosedMaterialHingeGraphPose,
    paper_thickness_bits: u64,
    analyzed_unordered_face_pairs: usize,
}

impl NativePositiveThicknessGraphGeometryProofV1 {
    #[must_use]
    pub fn is_for_geometry(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        pose: &ClosedMaterialHingeGraphPose,
        paper_thickness_mm: f64,
    ) -> bool {
        self.geometry.same_instance(geometry)
            && self.pose.same_instance(pose)
            && self.paper_thickness_bits == paper_thickness_mm.to_bits()
    }

    #[must_use]
    pub const fn analyzed_unordered_face_pairs(&self) -> usize {
        self.analyzed_unordered_face_pairs
    }

    #[must_use]
    pub fn paper_thickness_bits(&self) -> u64 {
        self.paper_thickness_bits
    }

    #[must_use]
    pub fn face_count(&self) -> usize {
        self.geometry.face_ids().len()
    }

    #[must_use]
    pub fn same_proof(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}

pub fn prove_positive_thickness_graph_geometry_v1(
    geometry: &MaterialHingeGraphGeometry,
    pose: &ClosedMaterialHingeGraphPose,
    paper_thickness_mm: f64,
    limits: PositiveThicknessGraphLimitsV1,
) -> Result<NativePositiveThicknessGraphGeometryProofV1, PositiveThicknessGraphProofErrorV1> {
    let face_count = geometry.face_ids().len();
    let checked_hinges = pose.closure_certificate().checked_hinges();
    let checked_hinge_set = checked_hinges.iter().copied().collect::<HashSet<_>>();
    if !(4..=49).contains(&face_count)
        || !paper_thickness_mm.is_finite()
        || paper_thickness_mm <= 0.0
        || checked_hinges.len() != geometry.hinges().len()
        || checked_hinge_set.len() != geometry.hinges().len()
        || !checked_hinges
            .iter()
            .all(|edge| geometry.hinges().iter().any(|hinge| hinge.edge() == *edge))
        || pose
            .hinge_angles()
            .as_slice()
            .iter()
            .any(|angle| angle.angle_degrees() >= 90.0)
    {
        return Err(PositiveThicknessGraphProofErrorV1::InvalidInput);
    }
    let pair_count = face_count
        .checked_mul(face_count - 1)
        .and_then(|value| value.checked_div(2))
        .ok_or(PositiveThicknessGraphProofErrorV1::ResourceLimit)?;
    if pair_count > limits.max_unordered_face_pairs {
        return Err(PositiveThicknessGraphProofErrorV1::ResourceLimit);
    }
    let half = BigRational::from_float(paper_thickness_mm)
        .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?
        / BigRational::from_integer(2.into());
    let radius = &half * BigRational::from_integer(16.into());
    let mut shared_feature_pairs = 0usize;
    for first_index in 0..face_count {
        let first = geometry.face_ids()[first_index];
        for second in &geometry.face_ids()[first_index + 1..] {
            let first_boundary = geometry
                .face_boundary_vertices(first)
                .filter(|boundary| boundary.len() >= 3)
                .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
            let second_boundary = geometry
                .face_boundary_vertices(*second)
                .filter(|boundary| boundary.len() >= 3)
                .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
            let shared = first_boundary
                .iter()
                .filter(|vertex| second_boundary.contains(vertex))
                .copied()
                .collect::<Vec<_>>();
            if shared.len() > 2 {
                return Err(PositiveThicknessGraphProofErrorV1::InvalidInput);
            }
            if !shared.is_empty() {
                shared_feature_pairs = shared_feature_pairs
                    .checked_add(1)
                    .ok_or(PositiveThicknessGraphProofErrorV1::ResourceLimit)?;
                if shared_feature_pairs > limits.max_shared_feature_pairs {
                    return Err(PositiveThicknessGraphProofErrorV1::ResourceLimit);
                }
            }
            let bounds = |face, boundary: &[ori_domain::VertexId]| {
                let transform = pose.face_transform(face)?;
                let normal = transform
                    .apply_vector(Point3::new(0.0, 1.0, 0.0).ok()?)
                    .ok()?;
                let mut lower = [f64::INFINITY; 3];
                let mut upper = [f64::NEG_INFINITY; 3];
                for vertex in boundary {
                    let point = transform
                        .apply_point(geometry.vertex_position(*vertex)?)
                        .ok()?;
                    for sign in [-1.0, 1.0] {
                        for (axis, value) in [
                            point.x() + sign * paper_thickness_mm * 0.5 * normal.x(),
                            point.y() + sign * paper_thickness_mm * 0.5 * normal.y(),
                            point.z() + sign * paper_thickness_mm * 0.5 * normal.z(),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            lower[axis] = lower[axis].min(value);
                            upper[axis] = upper[axis].max(value);
                        }
                    }
                }
                Some((lower, upper))
            };
            let (first_lower, first_upper) = bounds(first, first_boundary)
                .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
            let (second_lower, second_upper) = bounds(*second, second_boundary)
                .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
            let exact_lower: [BigRational; 3] = std::array::from_fn(|axis| {
                BigRational::from_float(first_lower[axis].max(second_lower[axis])).unwrap()
            });
            let exact_upper: [BigRational; 3] = std::array::from_fn(|axis| {
                BigRational::from_float(first_upper[axis].min(second_upper[axis])).unwrap()
            });
            if (0..3).any(|axis| exact_lower[axis] > exact_upper[axis]) {
                continue;
            }
            if !shared.is_empty()
                && pose
                    .hinge_angles()
                    .as_slice()
                    .iter()
                    .all(|angle| angle.angle_degrees().to_bits() == 0.0_f64.to_bits())
            {
                continue;
            }
            if shared.is_empty() {
                if first_boundary.len() > 64 || second_boundary.len() > 64 {
                    return Err(PositiveThicknessGraphProofErrorV1::ResourceLimit);
                }
                let world_polygon = |face, boundary: &[ori_domain::VertexId]| {
                    let transform = pose.face_transform(face)?;
                    boundary
                        .iter()
                        .map(|vertex| {
                            transform
                                .apply_point(geometry.vertex_position(*vertex)?)
                                .ok()
                        })
                        .collect::<Option<Vec<_>>>()
                };
                let first_polygon = world_polygon(first, first_boundary)
                    .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
                let second_polygon = world_polygon(*second, second_boundary)
                    .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
                let planar = first_polygon
                    .iter()
                    .chain(&second_polygon)
                    .all(|point| point.y().abs() <= f64::EPSILON);
                let separated = planar
                    && [&first_polygon, &second_polygon]
                        .into_iter()
                        .any(|polygon| {
                            (0..polygon.len()).any(|index| {
                                let start = polygon[index];
                                let end = polygon[(index + 1) % polygon.len()];
                                let axis = (end.z() - start.z(), start.x() - end.x());
                                let interval = |points: &[Point3]| {
                                    points.iter().fold(
                                        (None::<BigRational>, None::<BigRational>),
                                        |(lower, upper), point| {
                                            let value = BigRational::from_float(point.x()).unwrap()
                                                * BigRational::from_float(axis.0).unwrap()
                                                + BigRational::from_float(point.z()).unwrap()
                                                    * BigRational::from_float(axis.1).unwrap();
                                            (
                                                Some(lower.map_or_else(
                                                    || value.clone(),
                                                    |current| current.min(value.clone()),
                                                )),
                                                Some(upper.map_or_else(
                                                    || value.clone(),
                                                    |current| current.max(value.clone()),
                                                )),
                                            )
                                        },
                                    )
                                };
                                let (first_min, first_max) = interval(&first_polygon);
                                let (second_min, second_max) = interval(&second_polygon);
                                first_max < second_min || second_max < first_min
                            })
                        });
                let exact_point = |point: Point3| {
                    (
                        BigRational::from_float(point.x()).unwrap(),
                        BigRational::from_float(point.z()).unwrap(),
                    )
                };
                let orientation = |a: Point3, b: Point3, c: Point3| {
                    let (ax, ay) = exact_point(a);
                    let (bx, by) = exact_point(b);
                    let (cx, cy) = exact_point(c);
                    (bx - &ax) * (cy - &ay) - (by - &ay) * (cx - &ax)
                };
                let boundaries_cross = (0..first_polygon.len()).any(|first_index| {
                    let a = first_polygon[first_index];
                    let b = first_polygon[(first_index + 1) % first_polygon.len()];
                    (0..second_polygon.len()).any(|second_index| {
                        let c = second_polygon[second_index];
                        let d = second_polygon[(second_index + 1) % second_polygon.len()];
                        let ab_c = orientation(a, b, c);
                        let ab_d = orientation(a, b, d);
                        let cd_a = orientation(c, d, a);
                        let cd_b = orientation(c, d, b);
                        ab_c.signum() != ab_d.signum() && cd_a.signum() != cd_b.signum()
                    })
                });
                if separated || (planar && !boundaries_cross) {
                    continue;
                }
                return Err(PositiveThicknessGraphProofErrorV1::PairEvidenceUnavailable);
            }
            let shared_transform = pose
                .face_transform(first)
                .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
            let shared_points = shared
                .iter()
                .filter_map(|vertex| {
                    geometry
                        .vertex_position(*vertex)
                        .and_then(|point| shared_transform.apply_point(point).ok())
                })
                .collect::<Vec<_>>();
            if shared_points.len() != shared.len()
                || !(0..3).all(|axis| {
                    let values = shared_points.iter().map(|point| match axis {
                        0 => point.x(),
                        1 => point.y(),
                        _ => point.z(),
                    });
                    let lower = values.clone().fold(f64::INFINITY, f64::min);
                    let upper = values.fold(f64::NEG_INFINITY, f64::max);
                    exact_lower[axis] >= BigRational::from_float(lower).unwrap() - &radius
                        && exact_upper[axis] <= BigRational::from_float(upper).unwrap() + &radius
                })
            {
                return Err(PositiveThicknessGraphProofErrorV1::PairEvidenceUnavailable);
            }
        }
    }
    Ok(NativePositiveThicknessGraphGeometryProofV1 {
        identity: Arc::new(()),
        geometry: geometry.clone(),
        pose: pose.clone(),
        paper_thickness_bits: paper_thickness_mm.to_bits(),
        analyzed_unordered_face_pairs: pair_count,
    })
}

#[cfg(test)]
#[allow(clippy::duplicate_mod)]
#[path = "../../../test-support/four_bay_cycle.rs"]
mod four_bay_cycle_test_support;

#[cfg(test)]
mod tests {
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId};
    use ori_kinematics::{
        CanonicalHingeAngles, HingeAngle, MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
        TreeKinematicsLimits,
    };
    use ori_topology::{FaceExtractionInput, analyze_faces};

    use super::*;

    fn theta_shared_hinge_pattern() -> (CreasePattern, Paper) {
        let namespace = ProjectId::new();
        let points = [
            (-3.0, 0.0),
            (-1.0, -2.0),
            (1.0, -2.0),
            (3.0, 0.0),
            (1.0, 2.0),
            (-1.0, 2.0),
            (-1.0, 0.0),
            (1.0, 0.0),
        ];
        let vertices = points
            .into_iter()
            .enumerate()
            .map(|(index, (x, y))| Vertex {
                id: VertexId::derive_v5(namespace, &[index as u8]),
                position: Point2::new(x, y),
            })
            .collect::<Vec<_>>();
        let boundary = vertices[..6]
            .iter()
            .map(|vertex| vertex.id)
            .collect::<Vec<_>>();
        let mut edges = (0..6)
            .map(|index| Edge {
                id: ori_domain::EdgeId::derive_v5(namespace, &[0x10, index as u8]),
                start: boundary[index],
                end: boundary[(index + 1) % 6],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        for (index, (start, end)) in [(6, 0), (6, 1), (6, 5), (6, 7), (7, 2), (7, 3), (7, 4)]
            .into_iter()
            .enumerate()
        {
            edges.push(Edge {
                id: ori_domain::EdgeId::derive_v5(namespace, &[0x20, index as u8]),
                start: vertices[start].id,
                end: vertices[end].id,
                kind: if index == 3 {
                    EdgeKind::Mountain
                } else {
                    EdgeKind::Valley
                },
            });
        }
        (
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices: boundary,
                ..Paper::default()
            },
        )
    }

    #[test]
    fn real_theta_shared_hinge_static_proof_checks_every_face_pair_once() {
        let (pattern, mut paper) = theta_shared_hinge_pattern();
        paper.thickness_mm = 0.1;
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: ProjectId::new(),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        })
        .snapshot
        .expect("two physical vertices sharing one hinge form a theta dual graph");
        assert_eq!(topology.faces.len(), 6);
        assert_eq!(topology.hinge_adjacency.len(), 7);
        let geometry = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .unwrap();
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        assert_eq!(audit.closure_hinges().len(), 2);
        let angles = CanonicalHingeAngles::new(
            geometry
                .hinges()
                .iter()
                .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).unwrap())
                .collect(),
        )
        .unwrap();
        let pose = geometry
            .solve_closed(&audit, geometry.face_ids()[0], &angles, 0.0)
            .unwrap();
        let proof = prove_positive_thickness_graph_geometry_v1(
            &geometry,
            &pose,
            paper.thickness_mm,
            PositiveThicknessGraphLimitsV1::default(),
        )
        .expect("flat real theta positive-thickness proof");
        assert_eq!(proof.face_count(), 6);
        assert_eq!(proof.analyzed_unordered_face_pairs(), 15);
        assert_eq!(pose.closure_certificate().checked_hinges().len(), 7);
        assert!(matches!(
            prove_positive_thickness_graph_geometry_v1(
                &geometry,
                &pose,
                paper.thickness_mm,
                PositiveThicknessGraphLimitsV1 {
                    max_unordered_face_pairs: 14,
                    ..PositiveThicknessGraphLimitsV1::default()
                },
            ),
            Err(PositiveThicknessGraphProofErrorV1::ResourceLimit)
        ));
    }

    #[test]
    fn two_to_sixteen_cycle_cactus_proof_is_instance_bound_and_resource_bounded() {
        for group_count in [2, 3, 16] {
            let (pattern, paper, hinges) = match group_count {
                2 => super::four_bay_cycle_test_support::two_bay_rational_cycle_pattern(),
                3 => super::four_bay_cycle_test_support::three_bay_rational_cycle_pattern(),
                _ => super::four_bay_cycle_test_support::sixteen_bay_rational_cycle_pattern(),
            };
            let topology = analyze_faces(FaceExtractionInput {
                identity_namespace: ori_domain::ProjectId::new(),
                source_revision: 1,
                paper: &paper,
                pattern: &pattern,
            })
            .snapshot
            .expect("three-cycle cactus topology");
            let geometry = MaterialHingeGraphGeometry::prepare(
                &pattern,
                &paper,
                &topology,
                TreeKinematicsLimits::default(),
            )
            .unwrap();
            let audit =
                MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
                    .unwrap();
            let fixed = geometry.face_ids()[0];
            let mut angles = hinges
                .iter()
                .copied()
                .map(|edge| HingeAngle::new(edge, 0.0).unwrap())
                .collect::<Vec<_>>();
            angles.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
            let pose = geometry
                .solve_closed(
                    &audit,
                    fixed,
                    &CanonicalHingeAngles::new(angles).unwrap(),
                    1.0e-9,
                )
                .unwrap();
            let proof = prove_positive_thickness_graph_geometry_v1(
                &geometry,
                &pose,
                0.1,
                PositiveThicknessGraphLimitsV1::default(),
            )
            .expect("cactus exact-AABB proof");
            assert!(proof.is_for_geometry(&geometry, &pose, 0.1));
            let expected_pairs = geometry.face_ids().len() * (geometry.face_ids().len() - 1) / 2;
            assert_eq!(proof.analyzed_unordered_face_pairs(), expected_pairs);
            assert!(matches!(
                prove_positive_thickness_graph_geometry_v1(
                    &geometry,
                    &pose,
                    0.1,
                    PositiveThicknessGraphLimitsV1 {
                        max_unordered_face_pairs: expected_pairs - 1,
                        ..PositiveThicknessGraphLimitsV1::default()
                    },
                ),
                Err(PositiveThicknessGraphProofErrorV1::ResourceLimit)
            ));
            let foreign = geometry.clone();
            assert!(proof.is_for_geometry(&foreign, &pose, 0.1));
            assert!(proof.same_proof(&proof.clone()));
            if group_count == 2 {
                let mut aba_angles = hinges
                    .iter()
                    .copied()
                    .map(|edge| HingeAngle::new(edge, 0.0).unwrap())
                    .collect::<Vec<_>>();
                aba_angles.sort_unstable_by_key(|angle| angle.edge().canonical_bytes());
                let aba_pose = geometry
                    .solve_closed(
                        &audit,
                        fixed,
                        &CanonicalHingeAngles::new(aba_angles).unwrap(),
                        1.0e-9,
                    )
                    .unwrap();
                assert!(!proof.is_for_geometry(&geometry, &aba_pose, 0.1));
                let second_proof = prove_positive_thickness_graph_geometry_v1(
                    &geometry,
                    &pose,
                    0.1,
                    PositiveThicknessGraphLimitsV1::default(),
                )
                .unwrap();
                assert!(!proof.same_proof(&second_proof));
                assert!(matches!(
                    prove_positive_thickness_graph_geometry_v1(
                        &geometry,
                        &pose,
                        0.0,
                        PositiveThicknessGraphLimitsV1::default(),
                    ),
                    Err(PositiveThicknessGraphProofErrorV1::InvalidInput)
                ));
            }
            let separately_prepared = MaterialHingeGraphGeometry::prepare(
                &pattern,
                &paper,
                &topology,
                TreeKinematicsLimits::default(),
            )
            .unwrap();
            assert!(!proof.is_for_geometry(&separately_prepared, &pose, 0.1));
        }
    }
}
