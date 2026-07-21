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
    if !(3..=49).contains(&face_count)
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
    // A shared-hinge corridor is only contained in both incident face prisms
    // when the extrusion fits within an in-plane boundary feature.  Without
    // this premise an arbitrarily thick sheet was admitted at a flat hinge.
    let thickness_squared = BigRational::from_float(paper_thickness_mm)
        .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?
        .pow(2);
    let longest_hinge_span_squared = geometry
        .hinges()
        .iter()
        .map(|hinge| {
            let start = hinge.start();
            let end = hinge.end();
            let squared = [
                end.x() - start.x(),
                end.y() - start.y(),
                end.z() - start.z(),
            ]
            .into_iter()
            .map(|component| {
                BigRational::from_float(component)
                    .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)
                    .map(|value| value.pow(2))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum::<BigRational>();
            Ok(squared)
        })
        .collect::<Result<Vec<_>, PositiveThicknessGraphProofErrorV1>>()?
        .into_iter()
        .max()
        .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
    // Bind thickness to a local material hinge feature. Pattern-wide offsets
    // must not inflate this corridor and admit arbitrarily thick extrusion.
    let corridor_span_squared =
        longest_hinge_span_squared.clone() * BigRational::from_integer(64.into());
    if longest_hinge_span_squared <= BigRational::from_integer(0.into())
        || thickness_squared > corridor_span_squared
    {
        return Err(PositiveThicknessGraphProofErrorV1::PairEvidenceUnavailable);
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
                let prism_separated = {
                    let prism = |face, polygon: &[Point3]| {
                        let transform = pose.face_transform(face)?;
                        let normal = transform
                            .apply_vector(Point3::new(0.0, 1.0, 0.0).ok()?)
                            .ok()?;
                        let vertices = polygon
                            .iter()
                            .flat_map(|point| {
                                [-1.0, 1.0].map(|sign| {
                                    (
                                        point.x() + sign * paper_thickness_mm * 0.5 * normal.x(),
                                        point.y() + sign * paper_thickness_mm * 0.5 * normal.y(),
                                        point.z() + sign * paper_thickness_mm * 0.5 * normal.z(),
                                    )
                                })
                            })
                            .collect::<Vec<_>>();
                        let edges = (0..polygon.len())
                            .map(|index| {
                                let start = polygon[index];
                                let end = polygon[(index + 1) % polygon.len()];
                                (
                                    end.x() - start.x(),
                                    end.y() - start.y(),
                                    end.z() - start.z(),
                                )
                            })
                            .chain(std::iter::once((normal.x(), normal.y(), normal.z())))
                            .collect::<Vec<_>>();
                        Some((normal, vertices, edges))
                    };
                    let (first_normal, first_vertices, first_edges) = prism(first, &first_polygon)
                        .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
                    let (second_normal, second_vertices, second_edges) =
                        prism(*second, &second_polygon)
                            .ok_or(PositiveThicknessGraphProofErrorV1::InvalidInput)?;
                    let cross = |a: (f64, f64, f64), b: (f64, f64, f64)| {
                        (
                            a.1 * b.2 - a.2 * b.1,
                            a.2 * b.0 - a.0 * b.2,
                            a.0 * b.1 - a.1 * b.0,
                        )
                    };
                    let mut axes = vec![
                        (first_normal.x(), first_normal.y(), first_normal.z()),
                        (second_normal.x(), second_normal.y(), second_normal.z()),
                    ];
                    for edge in &first_edges {
                        axes.push(cross(*edge, axes[0]));
                    }
                    for edge in &second_edges {
                        axes.push(cross(*edge, axes[1]));
                    }
                    for first_edge in &first_edges {
                        for second_edge in &second_edges {
                            axes.push(cross(*first_edge, *second_edge));
                        }
                    }
                    axes.into_iter().any(|axis| {
                        let squared = axis.0 * axis.0 + axis.1 * axis.1 + axis.2 * axis.2;
                        if !squared.is_finite() || squared <= f64::EPSILON {
                            return false;
                        }
                        let interval = |vertices: &[(f64, f64, f64)]| {
                            vertices.iter().fold(
                                (None::<BigRational>, None::<BigRational>),
                                |(lower, upper), vertex| {
                                    let value = BigRational::from_float(vertex.0).unwrap()
                                        * BigRational::from_float(axis.0).unwrap()
                                        + BigRational::from_float(vertex.1).unwrap()
                                            * BigRational::from_float(axis.1).unwrap()
                                        + BigRational::from_float(vertex.2).unwrap()
                                            * BigRational::from_float(axis.2).unwrap();
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
                        let (first_min, first_max) = interval(&first_vertices);
                        let (second_min, second_max) = interval(&second_vertices);
                        first_max < second_min || second_max < first_min
                    })
                };
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
                if separated || prism_separated || (planar && !boundaries_cross) {
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
        CanonicalCycleScheduleV1, CanonicalHingeAngles, CycleScheduleEntryInputV1,
        CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, HingeAngle, MaterialHingeGraphAudit,
        MaterialHingeGraphGeometry, RationalCoefficientV1, TreeKinematicsLimits,
        admit_canonical_multi_hinge_path_candidate_v1,
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
                kind: if matches!(index, 0 | 3 | 5) {
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

    fn three_by_three_dense_cycle_pattern() -> (CreasePattern, Paper) {
        let namespace = ProjectId::new();
        let vertices = (0..4)
            .flat_map(|y| {
                (0..4).map(move |x| Vertex {
                    id: VertexId::derive_v5(namespace, &[0x31, y, x]),
                    position: Point2::new(f64::from(x), f64::from(y)),
                })
            })
            .collect::<Vec<_>>();
        let vertex = |x: usize, y: usize| vertices[y * 4 + x].id;
        let mut edges = Vec::new();
        for y in 0..4 {
            for x in 0..3 {
                edges.push(Edge {
                    id: ori_domain::EdgeId::derive_v5(namespace, &[0x32, y as u8, x as u8]),
                    start: vertex(x, y),
                    end: vertex(x + 1, y),
                    kind: if y == 0 || y == 3 {
                        EdgeKind::Boundary
                    } else {
                        EdgeKind::Mountain
                    },
                });
            }
        }
        for x in 0..4 {
            for y in 0..3 {
                edges.push(Edge {
                    id: ori_domain::EdgeId::derive_v5(namespace, &[0x33, x as u8, y as u8]),
                    start: vertex(x, y),
                    end: vertex(x, y + 1),
                    kind: if x == 0 || x == 3 {
                        EdgeKind::Boundary
                    } else {
                        EdgeKind::Valley
                    },
                });
            }
        }
        let boundary_vertices = (0..4)
            .map(|x| vertex(x, 0))
            .chain((1..4).map(|y| vertex(3, y)))
            .chain((0..3).rev().map(|x| vertex(x, 3)))
            .chain((1..3).rev().map(|y| vertex(0, y)))
            .collect();
        (
            CreasePattern { vertices, edges },
            Paper {
                boundary_vertices,
                thickness_mm: 0.1,
                ..Paper::default()
            },
        )
    }

    #[test]
    fn dense_rank_four_graph_constant_path_is_exact_resource_bound_and_instance_bound() {
        let (pattern, paper) = three_by_three_dense_cycle_pattern();
        let topology = analyze_faces(FaceExtractionInput {
            identity_namespace: ProjectId::new(),
            source_revision: 1,
            paper: &paper,
            pattern: &pattern,
        })
        .snapshot
        .expect("three-by-three material grid");
        assert_eq!(
            (topology.faces.len(), topology.hinge_adjacency.len()),
            (9, 12)
        );
        let geometry = MaterialHingeGraphGeometry::prepare(
            &pattern,
            &paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .unwrap();
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        assert_eq!(audit.closure_hinges().len(), 4, "cycle rank exceeds theta");
        let fixed = geometry.face_ids()[0];
        let schedule = CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            fixed,
            [0.0, 1.0],
            geometry
                .hinges()
                .iter()
                .map(|hinge| {
                    let moving = (hinge.end().z() - hinge.start().z()).abs() > 0.5;
                    CycleScheduleEntryInputV1 {
                        edge: hinge.edge(),
                        initial_angle_degrees_bits: if moving {
                            15.0_f64.to_bits()
                        } else {
                            0.0_f64.to_bits()
                        },
                        chebyshev_coefficients: if moving {
                            vec![
                                RationalCoefficientV1 {
                                    numerator: 0,
                                    denominator: 1,
                                },
                                RationalCoefficientV1 {
                                    numerator: 15,
                                    denominator: 1,
                                },
                            ]
                        } else {
                            vec![RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            }]
                        },
                    }
                })
                .collect(),
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        for progress in [0.0, 0.25, 0.5, 1.0] {
            let trial = schedule.evaluate(progress).unwrap();
            geometry
                .solve_closed(&audit, fixed, &trial, 1.0e-8)
                .unwrap_or_else(|error| panic!("dense grid closes at {progress}: {error:?}"));
        }
        assert_eq!(schedule.collective_profile_edges_v1().unwrap().len(), 6);
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                fixed,
                &schedule,
                1.0e-8,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 0,
                    max_leaves: 1,
                    max_work: 1,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .expect("stationary dense graph has exact one-leaf closure");
        let angles = schedule.evaluate(0.0).unwrap();
        let diagnosis = crate::diagnose_canonical_positive_thickness_cycle_schedule_path_v1(
            &geometry,
            &audit,
            fixed,
            &schedule,
            &closure,
            paper.thickness_mm,
            1,
        );
        assert!(diagnosis.continuous_certificate_model_id().is_some());
        assert_eq!(diagnosis.pair_work(), 36);
        assert_eq!(diagnosis.leaf_count(), 1);

        let pose = geometry.solve_closed(&audit, fixed, &angles, 0.0).unwrap();
        assert!(matches!(
            prove_positive_thickness_graph_geometry_v1(
                &geometry,
                &pose,
                paper.thickness_mm,
                PositiveThicknessGraphLimitsV1 {
                    max_unordered_face_pairs: 35,
                    max_shared_feature_pairs: 36
                },
            ),
            Err(PositiveThicknessGraphProofErrorV1::ResourceLimit)
        ));
        let proof = prove_positive_thickness_graph_geometry_v1(
            &geometry,
            &pose,
            paper.thickness_mm,
            PositiveThicknessGraphLimitsV1 {
                max_unordered_face_pairs: 36,
                max_shared_feature_pairs: 36,
            },
        )
        .unwrap();
        assert_eq!(proof.analyzed_unordered_face_pairs(), 36);
        assert!(!proof.is_for_geometry(
            &geometry,
            &pose,
            f64::from_bits(paper.thickness_mm.to_bits() + 1)
        ));
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
        let schedule = CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            geometry.face_ids()[0],
            [0.0, 1.0],
            geometry
                .hinges()
                .iter()
                .map(|hinge| CycleScheduleEntryInputV1 {
                    edge: hinge.edge(),
                    initial_angle_degrees_bits: if hinge.assignment()
                        == ori_topology::FoldAssignment::Mountain
                    {
                        15.0_f64.to_bits()
                    } else {
                        0.0_f64.to_bits()
                    },
                    chebyshev_coefficients: if hinge.assignment()
                        == ori_topology::FoldAssignment::Mountain
                    {
                        vec![
                            RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            },
                            RationalCoefficientV1 {
                                numerator: 15,
                                denominator: 1,
                            },
                        ]
                    } else {
                        vec![RationalCoefficientV1 {
                            numerator: 0,
                            denominator: 1,
                        }]
                    },
                })
                .collect(),
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        for progress in [0.25, 0.5, 1.0] {
            let scheduled = schedule.evaluate(progress).unwrap();
            geometry
                .solve_closed(&audit, geometry.face_ids()[0], &scheduled, 1.0e-8)
                .unwrap_or_else(|error| {
                    panic!("theta schedule must close at {progress}: {error:?}")
                });
        }
        let closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                geometry.face_ids()[0],
                &schedule,
                1.0e-8,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 0,
                    max_leaves: 1,
                    max_work: 1,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .expect("exact theta opposite-pair interval theorem");
        assert_eq!(closure.leaves().len(), 1);
        for one_short in [
            DyadicIntervalClosureLimitsV1 {
                max_depth: 0,
                max_leaves: 0,
                max_work: 1,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
            DyadicIntervalClosureLimitsV1 {
                max_depth: 0,
                max_leaves: 1,
                max_work: 0,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        ] {
            assert_eq!(
                geometry.prove_dyadic_schedule_closure_v1(
                    &audit,
                    geometry.face_ids()[0],
                    &schedule,
                    1.0e-8,
                    one_short,
                ),
                Err(ori_kinematics::DyadicIntervalClosureErrorV1::InvalidInput)
            );
        }
        let initial = schedule.evaluate(0.0).unwrap();
        let requested = schedule.evaluate(1.0).unwrap();
        let candidate =
            admit_canonical_multi_hinge_path_candidate_v1(schedule.clone(), &initial, &requested)
                .unwrap();
        for thickness in [0.1, 1.0, 3.0] {
            let continuous = crate::diagnose_scheduled_positive_thickness_cycle_path_v1(
                &geometry,
                &audit,
                geometry.face_ids()[0],
                &candidate,
                &closure,
                thickness,
                32,
            );
            assert!(continuous.continuous_certificate_model_id().is_some());
            assert!(
                crate::diagnose_scheduled_positive_thickness_cycle_path_v1(
                    &geometry,
                    &audit,
                    geometry.face_ids()[0],
                    &candidate,
                    &closure,
                    thickness,
                    0,
                )
                .continuous_certificate_model_id()
                .is_none()
            );
        }
        let collision_schedule = CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            geometry.face_ids()[0],
            [0.0, 1.0],
            geometry
                .hinges()
                .iter()
                .map(|hinge| {
                    let moves = hinge.assignment() == ori_topology::FoldAssignment::Mountain;
                    CycleScheduleEntryInputV1 {
                        edge: hinge.edge(),
                        initial_angle_degrees_bits: if moves {
                            45.0_f64.to_bits()
                        } else {
                            0.0_f64.to_bits()
                        },
                        chebyshev_coefficients: if moves {
                            vec![
                                RationalCoefficientV1 {
                                    numerator: 0,
                                    denominator: 1,
                                },
                                RationalCoefficientV1 {
                                    numerator: 45,
                                    denominator: 1,
                                },
                            ]
                        } else {
                            vec![RationalCoefficientV1 {
                                numerator: 0,
                                denominator: 1,
                            }]
                        },
                    }
                })
                .collect(),
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        let collision_closure = geometry
            .prove_dyadic_schedule_closure_v1(
                &audit,
                geometry.face_ids()[0],
                &collision_schedule,
                1.0e-8,
                DyadicIntervalClosureLimitsV1 {
                    max_depth: 0,
                    max_leaves: 1,
                    max_work: 1,
                    schedule_limits: CycleScheduleLimitsV1::default(),
                },
            )
            .unwrap();
        let collision_initial = collision_schedule.evaluate(0.0).unwrap();
        let collision_target = collision_schedule.evaluate(1.0).unwrap();
        let collision_candidate = admit_canonical_multi_hinge_path_candidate_v1(
            collision_schedule,
            &collision_initial,
            &collision_target,
        )
        .unwrap();
        assert!(
            crate::diagnose_scheduled_positive_thickness_cycle_path_v1(
                &geometry,
                &audit,
                geometry.face_ids()[0],
                &collision_candidate,
                &collision_closure,
                0.1,
                32,
            )
            .continuous_certificate_model_id()
            .is_none(),
            "the thickness singularity at 90 degrees must issue no swept certificate"
        );
        for thickness in [0.1, 1.0, 3.0] {
            let proof = prove_positive_thickness_graph_geometry_v1(
                &geometry,
                &pose,
                thickness,
                PositiveThicknessGraphLimitsV1::default(),
            )
            .expect("flat real theta positive-thickness proof");
            assert_eq!(proof.face_count(), 6);
            assert_eq!(proof.analyzed_unordered_face_pairs(), 15);
            assert_eq!(proof.paper_thickness_bits(), thickness.to_bits());
            assert!(!proof.is_for_geometry(
                &geometry,
                &pose,
                f64::from_bits(thickness.to_bits() + 1),
            ));
        }
        assert_eq!(pose.closure_certificate().checked_hinges().len(), 7);
        let shared_hinge = geometry
            .hinges()
            .iter()
            .find(|hinge| {
                geometry
                    .hinges()
                    .iter()
                    .filter(|candidate| {
                        candidate.start() == hinge.start() || candidate.end() == hinge.start()
                    })
                    .count()
                    >= 4
                    && geometry
                        .hinges()
                        .iter()
                        .filter(|candidate| {
                            candidate.start() == hinge.end() || candidate.end() == hinge.end()
                        })
                        .count()
                        >= 4
            })
            .expect("unique hinge joining both degree-four physical vertices")
            .edge();
        let damaged_angles = CanonicalHingeAngles::new(
            geometry
                .hinges()
                .iter()
                .map(|hinge| {
                    HingeAngle::new(
                        hinge.edge(),
                        if hinge.edge() == shared_hinge {
                            1.0
                        } else {
                            0.0
                        },
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        assert!(
            geometry
                .solve_closed(&audit, geometry.face_ids()[0], &damaged_angles, 0.0)
                .is_err(),
            "damaged shared theta hinge must issue neither closed pose nor thickness proof"
        );
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
            for thickness in [1.0, 3.0] {
                assert!(
                    prove_positive_thickness_graph_geometry_v1(
                        &geometry,
                        &pose,
                        thickness,
                        PositiveThicknessGraphLimitsV1::default(),
                    )
                    .is_ok(),
                    "cactus group {group_count} supports {thickness} mm"
                );
            }
            assert_eq!(
                prove_positive_thickness_graph_geometry_v1(
                    &geometry,
                    &pose,
                    10_000.0,
                    PositiveThicknessGraphLimitsV1::default(),
                )
                .unwrap_err(),
                PositiveThicknessGraphProofErrorV1::PairEvidenceUnavailable
            );
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
