//! Shared general-N Miura fixture and registry construction for clearance V2 tests.

use std::collections::{BTreeMap, BTreeSet};

use super::validation::enumerate_canonical_cross_block_pairs_v2;
use super::*;
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalHingeAngles, CommonArticulationBlockClosureSetInputV2,
    CommonArticulationBlockClosureSetLimitsV2, CommonArticulationBlockClosureSetV2,
    CommonArticulationWholeParentClosureInputV2, CommonArticulationWholeParentClosureLimitsV2,
    CommonArticulationWholeParentClosureV2, CycleScheduleEntryInputV1, CycleScheduleLimitsV1,
    DyadicIntervalClosureLimitsV1, HingeAngle, RationalCoefficientV1, TreeKinematicsLimits,
    prove_common_articulation_block_closure_set_v2, prove_common_articulation_pose_authority_v2,
    prove_common_articulation_whole_parent_closure_v2,
};
use ori_topology::{FaceExtractionInput, analyze_faces};

pub(crate) struct MiuraFixtureV2 {
    /// Retained exact source only for bounded test-only provenance mutation.
    /// The helper below regenerates topology rather than retaining or cloning
    /// a second unbounded topology snapshot.
    pub(crate) pattern: CreasePattern,
    pub(crate) paper: Paper,
    pub(crate) geometry: MaterialHingeGraphGeometry,
    pub(crate) audit: MaterialHingeGraphAudit,
    pub(crate) pose: ClosedMaterialHingeGraphPose,
    pub(crate) decomposition: CanonicalMaterialEdgeBlockDecompositionV2,
    pub(crate) common_pose: CommonArticulationPoseAuthorityV2,
    pub(crate) profile: CommonArticulationResourceProfileV2,
    pub(crate) parent_fixed_face: FaceId,
    pub(crate) parent_schedule: CanonicalCycleScheduleV1,
    pub(crate) closure_tolerance: f64,
    pub(crate) block_closure_set: CommonArticulationBlockClosureSetV2,
    pub(crate) whole_parent_closure: CommonArticulationWholeParentClosureV2,
    pub(crate) whole_parent_closure_limits: CommonArticulationWholeParentClosureLimitsV2,
    pub(crate) pairs: Vec<CommonArticulationCrossBlockFacePairV2>,
}

impl MiuraFixtureV2 {
    /// Rebuilds the N=33 geometry after changing only material-component
    /// origin metadata. Pattern, paper, source revision, face registry, and
    /// fold-model fingerprint remain those of this fixture.
    pub(crate) fn geometry_with_tampered_component_origin_for_test(
        &self,
        foreign_origin: ProjectId,
    ) -> MaterialHingeGraphGeometry {
        let namespace = self
            .geometry
            .source_identity_namespace_v1()
            .expect("canonical test geometry namespace");
        let mut topology = analyze_faces(FaceExtractionInput {
            identity_namespace: namespace,
            source_revision: self
                .geometry
                .source_revision_v1()
                .expect("canonical test geometry revision"),
            paper: &self.paper,
            pattern: &self.pattern,
        })
        .snapshot
        .expect("canonical N=33 topology");
        for component in &mut topology.material_components {
            component.sheet_origin = foreign_origin;
        }
        MaterialHingeGraphGeometry::prepare(
            &self.pattern,
            &self.paper,
            &topology,
            TreeKinematicsLimits::default(),
        )
        .expect("material-origin metadata does not invalidate geometry observation")
    }

    pub(crate) fn input(&self) -> CommonArticulationClearanceInputV2<'_> {
        CommonArticulationClearanceInputV2 {
            geometry: &self.geometry,
            audit: &self.audit,
            pose: &self.pose,
            decomposition: &self.decomposition,
            common_pose: &self.common_pose,
            parent_fixed_face: self.parent_fixed_face,
            parent_schedule: &self.parent_schedule,
            profile: &self.profile,
            paper_thickness_mm: 0.1,
            closure_tolerance: self.closure_tolerance,
            block_closure_set: &self.block_closure_set,
            whole_parent_closure: &self.whole_parent_closure,
            whole_parent_closure_limits: self.whole_parent_closure_limits,
            submitted_cross_block_pairs: &self.pairs,
        }
    }

    pub(crate) fn revalidation_input(&self) -> CommonArticulationClearanceRevalidationInputV2<'_> {
        CommonArticulationClearanceRevalidationInputV2 {
            geometry: &self.geometry,
            audit: &self.audit,
            pose: &self.pose,
            decomposition: &self.decomposition,
            common_pose: &self.common_pose,
            parent_fixed_face: self.parent_fixed_face,
            parent_schedule: &self.parent_schedule,
            profile: &self.profile,
            paper_thickness_mm: 0.1,
            closure_tolerance: self.closure_tolerance,
            block_closure_set: &self.block_closure_set,
            whole_parent_closure: &self.whole_parent_closure,
            whole_parent_closure_limits: self.whole_parent_closure_limits,
        }
    }
}

pub(crate) fn miura_fixture_v2() -> MiuraFixtureV2 {
    miura_fixture_v2_with_profile(33, 33)
}

pub(crate) fn golden_n33_miura_fixture_v2() -> MiuraFixtureV2 {
    miura_fixture_v2_with_profile_and_namespace(
        33,
        33,
        ProjectId::schema_namespace([
            0x4f, 0x52, 0x49, 0x47, 0x41, 0x4d, 0x49, 0x32, 0x5f, 0x4e, 0x5f, 0x56, 0x32, 0, 0, 2,
        ]),
    )
}

pub(super) fn miura_fixture_v2_with_profile(
    configured_max_blocks: usize,
    actual_block_count: usize,
) -> MiuraFixtureV2 {
    miura_fixture_v2_with_profile_and_namespace(
        configured_max_blocks,
        actual_block_count,
        ProjectId::new(),
    )
}

pub(super) fn miura_fixture_v2_with_profile_and_namespace(
    configured_max_blocks: usize,
    actual_block_count: usize,
    namespace: ProjectId,
) -> MiuraFixtureV2 {
    let cells = (0..actual_block_count)
        .flat_map(|index| {
            let x = i8::try_from(index.checked_mul(2).expect("block x multiplication"))
                .expect("general-N fixture block x fits i8");
            let y = if index % 2 == 0 { 0_i8 } else { -2_i8 };
            (x..=x + 2).flat_map(move |x| (y..=y + 2).map(move |y| (x, y)))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let (pattern, paper) = miura_pattern_v2(&cells, namespace);
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("general-N Miura topology");
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("general-N Miura geometry");
    let audit = MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
        .expect("general-N Miura audit");
    let profile = CommonArticulationResourceProfileV2::for_canonical_miura_3x3_v2(
        configured_max_blocks,
        actual_block_count,
    )
    .expect("general-N profile");
    let decomposition = geometry
        .decompose_canonical_edge_blocks_with_profile_v2(&audit, &profile)
        .expect("general-N decomposition");
    let angles = CanonicalHingeAngles::new(
        geometry
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).expect("zero angle"))
            .collect(),
    )
    .expect("canonical zero angles");
    let pose = geometry
        .solve_closed(&audit, geometry.face_ids()[0], &angles, 0.0)
        .expect("general-N closed pose");
    let common_pose = prove_common_articulation_pose_authority_v2(CommonArticulationPoseInputV2 {
        geometry: &geometry,
        pose: &pose,
        decomposition: &decomposition,
        paper_thickness_mm: 0.1,
        profile: &profile,
    })
    .expect("general-N pose authority");
    let parent_fixed_face = geometry.face_ids()[0];
    let parent_schedule = CanonicalCycleScheduleV1::prepare(
        &geometry,
        &audit,
        parent_fixed_face,
        [0.0, 1.0],
        zero_schedule_entries_v2(&geometry),
        parent_schedule_limits_v2(&geometry),
    )
    .expect("general-N parent schedule");
    let closure_tolerance = 1.0e-9;
    let block_closure_set_limits = block_closure_set_limits_v2(configured_max_blocks);
    let block_closure_set =
        prove_common_articulation_block_closure_set_v2(CommonArticulationBlockClosureSetInputV2 {
            geometry: &geometry,
            audit: &audit,
            pose: &pose,
            parent_fixed_face,
            parent_schedule: &parent_schedule,
            decomposition: &decomposition,
            common_pose: &common_pose,
            paper_thickness_mm: 0.1,
            closure_tolerance,
            profile: &profile,
            limits: block_closure_set_limits,
        })
        .expect("general-N block closure set");
    let whole_parent_closure_limits = CommonArticulationWholeParentClosureLimitsV2 {
        block_closure_set_limits,
        max_parent_schedule_bytes: 65_536,
        max_parent_closure_bytes: 65_536,
        max_parent_closure_leaves: 1,
        parent_closure_limits: DyadicIntervalClosureLimitsV1 {
            max_depth: 0,
            max_leaves: 1,
            max_work: 1,
            schedule_limits: parent_schedule_limits_v2(&geometry),
        },
    };
    let whole_parent_closure = prove_common_articulation_whole_parent_closure_v2(
        CommonArticulationWholeParentClosureInputV2 {
            geometry: &geometry,
            audit: &audit,
            pose: &pose,
            parent_fixed_face,
            parent_schedule: &parent_schedule,
            decomposition: &decomposition,
            common_pose: &common_pose,
            paper_thickness_mm: 0.1,
            closure_tolerance,
            profile: &profile,
            block_closure_set: &block_closure_set,
            limits: whole_parent_closure_limits,
        },
    )
    .expect("general-N whole parent closure");
    let pairs = enumerate_canonical_cross_block_pairs_v2(
        &decomposition,
        profile.actual_v2().raw_cross_block_pair_candidates_v2(),
        profile.actual_v2().canonical_cross_block_pairs_v2(),
        &mut || Ok(()),
    )
    .expect("canonical pair registry");
    MiuraFixtureV2 {
        pattern,
        paper,
        geometry,
        audit,
        pose,
        decomposition,
        common_pose,
        profile,
        parent_fixed_face,
        parent_schedule,
        closure_tolerance,
        block_closure_set,
        whole_parent_closure,
        whole_parent_closure_limits,
        pairs,
    }
}

fn parent_schedule_limits_v2(geometry: &MaterialHingeGraphGeometry) -> CycleScheduleLimitsV1 {
    CycleScheduleLimitsV1 {
        max_hinges: geometry.hinges().len(),
        max_degree: 0,
        max_coefficient_bits: 1,
        max_work: geometry.hinges().len(),
    }
}

fn zero_schedule_entries_v2(
    geometry: &MaterialHingeGraphGeometry,
) -> Vec<CycleScheduleEntryInputV1> {
    let mut entries = geometry
        .hinges()
        .iter()
        .map(|hinge| CycleScheduleEntryInputV1 {
            edge: hinge.edge(),
            initial_angle_degrees_bits: 0.0_f64.to_bits(),
            chebyshev_coefficients: vec![RationalCoefficientV1 {
                numerator: 0,
                denominator: 1,
            }],
        })
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
    entries
}

fn block_closure_set_limits_v2(
    configured_max_blocks: usize,
) -> CommonArticulationBlockClosureSetLimitsV2 {
    const PER_BLOCK_BYTES: usize = 8_192;
    CommonArticulationBlockClosureSetLimitsV2 {
        max_blocks: configured_max_blocks,
        max_parent_schedule_bytes: 65_536,
        max_block_schedule_bytes: PER_BLOCK_BYTES,
        max_total_block_schedule_bytes: configured_max_blocks * PER_BLOCK_BYTES,
        max_block_closure_bytes: PER_BLOCK_BYTES,
        max_total_block_closure_bytes: configured_max_blocks * PER_BLOCK_BYTES,
        max_total_closure_leaves: configured_max_blocks,
        per_block_closure_limits: DyadicIntervalClosureLimitsV1 {
            max_depth: 0,
            max_leaves: 1,
            max_work: 1,
            schedule_limits: CycleScheduleLimitsV1 {
                max_hinges: 12,
                max_degree: 0,
                max_coefficient_bits: 1,
                max_work: 12,
            },
        },
    }
}

fn miura_pattern_v2(cells: &[(i8, i8)], namespace: ProjectId) -> (CreasePattern, Paper) {
    let mut points = BTreeSet::new();
    let mut incidence = BTreeMap::<((i8, i8), (i8, i8)), (usize, (i8, i8), (i8, i8))>::new();
    for &(x, y) in cells {
        let corners = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1)];
        points.extend(corners);
        for index in 0..4 {
            let start = corners[index];
            let end = corners[(index + 1) % 4];
            let key = if start < end {
                (start, end)
            } else {
                (end, start)
            };
            incidence
                .entry(key)
                .and_modify(|entry| entry.0 += 1)
                .or_insert((1, start, end));
        }
    }
    let vertices = points
        .iter()
        .map(|&(x, y)| Vertex {
            id: VertexId::derive_v5(namespace, &[0xc1, (x + 4) as u8, (y + 4) as u8]),
            position: Point2::new(f64::from(x) * 20.0, f64::from(y) * 20.0),
        })
        .collect::<Vec<_>>();
    let vertex = |point: (i8, i8)| {
        vertices[points
            .iter()
            .position(|candidate| *candidate == point)
            .expect("cell corner")]
        .id
    };
    let edges = incidence
        .iter()
        .map(|(&(first, second), &(count, start, end))| Edge {
            id: EdgeId::derive_v5(
                namespace,
                &[
                    0xc2,
                    (first.0 + 4) as u8,
                    (first.1 + 4) as u8,
                    (second.0 + 4) as u8,
                    (second.1 + 4) as u8,
                ],
            ),
            start: vertex(start),
            end: vertex(end),
            kind: if count == 1 {
                EdgeKind::Boundary
            } else if first.1 == second.1 {
                EdgeKind::Mountain
            } else if first.1.rem_euclid(2) == 0 {
                EdgeKind::Valley
            } else {
                EdgeKind::Mountain
            },
        })
        .collect::<Vec<_>>();
    let directed = incidence
        .values()
        .filter(|(count, _, _)| *count == 1)
        .map(|(_, start, end)| (*start, *end))
        .collect::<Vec<_>>();
    let mut boundary = vec![directed[0].0];
    while boundary.len() < directed.len() {
        let cursor = *boundary.last().expect("boundary start");
        boundary.push(
            directed
                .iter()
                .find(|(start, _)| *start == cursor)
                .expect("next boundary edge")
                .1,
        );
    }
    let boundary_vertices = boundary.into_iter().map(vertex).collect();
    (
        CreasePattern { vertices, edges },
        Paper {
            boundary_vertices,
            thickness_mm: 0.1,
            ..Paper::default()
        },
    )
}
