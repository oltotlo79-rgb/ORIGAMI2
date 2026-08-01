use crate::{
    CommonArticulationClearanceExtensionInputV1, CommonArticulationClearanceExtensionLimitsV1,
    CommonArticulationClearanceExtensionOutcomeV1,
    CommonArticulationClearanceExtensionPrerequisiteV1,
    CommonArticulationClearanceExtensionRevalidationInputV1,
    CommonArticulationCrossBlockFacePairV1, PositiveThicknessContinuousCertificateV1,
    certify_canonical_positive_thickness_cycle_schedule_path_v1,
    issue_common_articulation_clearance_extension_prerequisite_v1,
};
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalEdgeBlockLimitsV1, CanonicalHingeAngles,
    CanonicalMaterialEdgeBlockDecompositionV1, ClosedMaterialHingeGraphPose,
    CommonArticulationPoseExtensionAuthorityV1, CommonArticulationPoseExtensionInputV1,
    CommonArticulationPoseExtensionLimitsV1, CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1,
    DyadicMaterialHingeIntervalClosureCertificateV1, HalfAngleRationalEntryInputV1, HingeAngle,
    MaterialHingeGraphAudit, MaterialHingeGraphGeometry, RationalCoefficientV1,
    TreeKinematicsLimits, prove_common_articulation_pose_extension_authority_v1,
};
use ori_topology::{FaceExtractionInput, analyze_faces};

pub(crate) struct ExtensionClearanceFixtureV1 {
    pub(crate) geometry: MaterialHingeGraphGeometry,
    pub(crate) audit: MaterialHingeGraphAudit,
    pub(crate) pose: ClosedMaterialHingeGraphPose,
    pub(crate) decomposition: CanonicalMaterialEdgeBlockDecompositionV1,
    pub(crate) schedule: CanonicalCycleScheduleV1,
    pub(crate) closure: DyadicMaterialHingeIntervalClosureCertificateV1,
    pub(crate) pairs: Vec<CommonArticulationCrossBlockFacePairV1>,
    pub(crate) positive: PositiveThicknessContinuousCertificateV1,
    pub(crate) paper_thickness_mm: f64,
}

impl ExtensionClearanceFixtureV1 {
    pub(crate) fn pose_authority_v1(
        &self,
        configured_max_blocks: usize,
    ) -> CommonArticulationPoseExtensionAuthorityV1 {
        prove_common_articulation_pose_extension_authority_v1(
            CommonArticulationPoseExtensionInputV1 {
                geometry: &self.geometry,
                pose: &self.pose,
                decomposition: &self.decomposition,
                paper_thickness_mm: self.paper_thickness_mm,
                limits: pose_extension_limits_v1(configured_max_blocks),
            },
        )
        .expect("extension pose authority")
    }

    pub(crate) fn input<'a>(
        &'a self,
        common_pose: &'a CommonArticulationPoseExtensionAuthorityV1,
        common_pose_limits: CommonArticulationPoseExtensionLimitsV1,
        submitted_cross_block_pairs: &'a [CommonArticulationCrossBlockFacePairV1],
        whole_parent_continuous: Option<PositiveThicknessContinuousCertificateV1>,
        limits: CommonArticulationClearanceExtensionLimitsV1,
    ) -> CommonArticulationClearanceExtensionInputV1<'a> {
        CommonArticulationClearanceExtensionInputV1 {
            geometry: &self.geometry,
            audit: &self.audit,
            pose: &self.pose,
            decomposition: &self.decomposition,
            common_pose,
            common_pose_limits,
            schedule: &self.schedule,
            schedule_limits: CycleScheduleLimitsV1::default(),
            closure: &self.closure,
            paper_thickness_mm: self.paper_thickness_mm,
            submitted_cross_block_pairs,
            whole_parent_continuous,
            limits,
        }
    }

    pub(crate) fn revalidation_input<'a>(
        &'a self,
        common_pose: &'a CommonArticulationPoseExtensionAuthorityV1,
        common_pose_limits: CommonArticulationPoseExtensionLimitsV1,
        limits: CommonArticulationClearanceExtensionLimitsV1,
    ) -> CommonArticulationClearanceExtensionRevalidationInputV1<'a> {
        CommonArticulationClearanceExtensionRevalidationInputV1 {
            geometry: &self.geometry,
            audit: &self.audit,
            pose: &self.pose,
            decomposition: &self.decomposition,
            common_pose,
            common_pose_limits,
            schedule: &self.schedule,
            schedule_limits: CycleScheduleLimitsV1::default(),
            closure: &self.closure,
            paper_thickness_mm: self.paper_thickness_mm,
            limits,
        }
    }

    pub(crate) fn canonical_edge_partition_v1(&self) -> Vec<Vec<EdgeId>> {
        self.decomposition
            .blocks()
            .iter()
            .map(|block| {
                block
                    .geometry()
                    .hinges()
                    .iter()
                    .map(|hinge| hinge.edge())
                    .collect()
            })
            .collect()
    }
}

fn coefficient_v1(numerator: i64, denominator: u64) -> RationalCoefficientV1 {
    RationalCoefficientV1 {
        numerator,
        denominator,
    }
}

fn prepare_extension_schedule_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
) -> (
    CanonicalCycleScheduleV1,
    DyadicMaterialHingeIntervalClosureCertificateV1,
) {
    let entries = geometry
        .hinges()
        .iter()
        .map(|hinge| HalfAngleRationalEntryInputV1 {
            edge: hinge.edge(),
            u_domain: [coefficient_v1(0, 1), coefficient_v1(1, 1)],
            numerator_power_coefficients: vec![coefficient_v1(0, 1), coefficient_v1(0, 1)],
            denominator_power_coefficients: vec![coefficient_v1(64, 1)],
        })
        .collect();
    let schedule = CanonicalCycleScheduleV1::prepare_half_angle_rational(
        geometry,
        audit,
        fixed_face,
        entries,
        CycleScheduleLimitsV1::default(),
    )
    .expect("canonical extension strip schedule");
    let closure = geometry
        .prove_dyadic_schedule_closure_v1(
            audit,
            fixed_face,
            &schedule,
            1.0e-9,
            DyadicIntervalClosureLimitsV1 {
                max_depth: 8,
                max_leaves: 256,
                max_work: 1_000_000,
                schedule_limits: CycleScheduleLimitsV1::default(),
            },
        )
        .expect("canonical extension strip closure");
    (schedule, closure)
}

fn independent_cross_block_pairs_v1(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
) -> Vec<CommonArticulationCrossBlockFacePairV1> {
    let blocks = decomposition.blocks();
    let mut pairs = Vec::new();
    for first in 0..blocks.len() {
        for second in first + 1..blocks.len() {
            for first_face in blocks[first].geometry().face_ids() {
                for second_face in blocks[second].geometry().face_ids() {
                    if let Some(pair) =
                        CommonArticulationCrossBlockFacePairV1::new(*first_face, *second_face)
                    {
                        pairs.push(pair);
                    }
                }
            }
        }
    }
    pairs.sort_unstable_by(|left, right| {
        left.first()
            .canonical_bytes()
            .cmp(&right.first().canonical_bytes())
            .then_with(|| {
                left.second()
                    .canonical_bytes()
                    .cmp(&right.second().canonical_bytes())
            })
    });
    pairs.dedup();
    pairs
}

pub(crate) fn prepare_extension_clearance_fixture_v1(
    block_count: usize,
) -> ExtensionClearanceFixtureV1 {
    assert!((10..=32).contains(&block_count));
    let namespace = ProjectId::new();
    let face_count = block_count + 1;
    let bottom = (0..=face_count)
        .map(|_| VertexId::new())
        .collect::<Vec<_>>();
    let top = (0..=face_count)
        .map(|_| VertexId::new())
        .collect::<Vec<_>>();
    let vertices = bottom
        .iter()
        .zip(&top)
        .enumerate()
        .flat_map(|(x, (&bottom, &top))| {
            let x = x as f64 * 10.0;
            [
                Vertex {
                    id: bottom,
                    position: Point2::new(x, 0.0),
                },
                Vertex {
                    id: top,
                    position: Point2::new(x, 10.0),
                },
            ]
        })
        .collect::<Vec<_>>();
    let boundary = bottom
        .iter()
        .copied()
        .chain(top.iter().rev().copied())
        .collect::<Vec<_>>();
    let mut edges = (0..boundary.len())
        .map(|index| Edge {
            id: EdgeId::new(),
            start: boundary[index],
            end: boundary[(index + 1) % boundary.len()],
            kind: EdgeKind::Boundary,
        })
        .collect::<Vec<_>>();
    edges.extend((1..=block_count).map(|x| Edge {
        id: EdgeId::new(),
        start: bottom[x],
        end: top[x],
        kind: EdgeKind::Mountain,
    }));
    let pattern = CreasePattern { vertices, edges };
    let paper_thickness_mm = 0.1;
    let paper = Paper {
        boundary_vertices: boundary,
        thickness_mm: paper_thickness_mm,
        ..Paper::default()
    };
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("extension strip topology");
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("extension strip geometry");
    let audit = MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
        .expect("extension strip audit");
    assert_eq!(geometry.face_ids().len(), face_count);
    assert_eq!(geometry.hinges().len(), block_count);
    let decomposition = geometry
        .decompose_canonical_edge_blocks_v1(
            &audit,
            CanonicalEdgeBlockLimitsV1 {
                max_blocks: block_count,
                max_faces_per_block: 2,
                max_hinges_per_block: 2,
            },
        )
        .expect("canonical extension strip decomposition");
    assert_eq!(decomposition.blocks().len(), block_count);
    let fixed_face = geometry.face_ids()[0];
    let angles = CanonicalHingeAngles::new(
        geometry
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).expect("zero extension strip angle"))
            .collect(),
    )
    .expect("canonical extension strip angles");
    let pose = geometry
        .solve_closed(&audit, fixed_face, &angles, 0.0)
        .expect("closed extension strip pose");
    let (schedule, closure) = prepare_extension_schedule_v1(&geometry, &audit, fixed_face);
    let positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &geometry,
        &audit,
        fixed_face,
        &schedule,
        &closure,
        paper_thickness_mm,
        16,
    )
    .expect("whole-parent extension positive-thickness certificate");
    let pairs = independent_cross_block_pairs_v1(&decomposition);
    assert!(!pairs.is_empty());
    ExtensionClearanceFixtureV1 {
        geometry,
        audit,
        pose,
        decomposition,
        schedule,
        closure,
        pairs,
        positive,
        paper_thickness_mm,
    }
}

pub(crate) fn pose_extension_limits_v1(
    configured_max_blocks: usize,
) -> CommonArticulationPoseExtensionLimitsV1 {
    CommonArticulationPoseExtensionLimitsV1::with_max_blocks_v1(configured_max_blocks)
        .expect("valid pose extension cap")
}

pub(crate) fn clearance_extension_limits_v1(
    configured_max_blocks: usize,
) -> CommonArticulationClearanceExtensionLimitsV1 {
    CommonArticulationClearanceExtensionLimitsV1::with_max_blocks_v1(configured_max_blocks)
        .expect("valid clearance extension cap")
}

pub(crate) fn raw_pair_candidate_count_v1(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
) -> usize {
    let blocks = decomposition.blocks();
    (0..blocks.len())
        .flat_map(|first| (first + 1..blocks.len()).map(move |second| (first, second)))
        .map(|(first, second)| {
            blocks[first].geometry().face_ids().len() * blocks[second].geometry().face_ids().len()
        })
        .sum()
}

pub(crate) fn issue_extension_clearance_v1<'a>(
    fixture: &'a ExtensionClearanceFixtureV1,
    common_pose: &'a CommonArticulationPoseExtensionAuthorityV1,
    configured_max_blocks: usize,
) -> Box<CommonArticulationClearanceExtensionPrerequisiteV1> {
    let pose_limits = pose_extension_limits_v1(configured_max_blocks);
    let limits = clearance_extension_limits_v1(configured_max_blocks);
    match issue_common_articulation_clearance_extension_prerequisite_v1(fixture.input(
        common_pose,
        pose_limits,
        &fixture.pairs,
        Some(fixture.positive.clone()),
        limits,
    ))
    .expect("extension clearance outcome")
    {
        CommonArticulationClearanceExtensionOutcomeV1::Certified(authority) => authority,
        CommonArticulationClearanceExtensionOutcomeV1::Unsupported(_) => {
            panic!("whole-parent extension certificate must close the prerequisite")
        }
    }
}
