use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::{Duration, Instant},
};

use super::*;
use crate::{
    BLOCK_COMPOSITION_LIMIT_V1, BlockUnionCompletenessInputV1,
    COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_MODEL_ID_V1,
    COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_MODEL_ID_V1,
    CommonArticulationBlockComposedPathAuthorityV1, CommonArticulationBlockComposedPathErrorV1,
    CommonArticulationBlockComposedPathInputV1, CommonArticulationContinuousLayerPathAuthorityV1,
    CommonArticulationContinuousLayerPathErrorV1, CommonArticulationContinuousLayerPathInputV1,
    CommonArticulationContinuousLayerPathRevalidationInputV1,
    CompleteMultiBlockPositiveLayerAuthorityV1, GeneralCellTransportInputV1,
    GeneralCellTransportLimitsV1, GeneralMultiFaceCellTransportProofV1, MultiBlockClosureInputV1,
    MultiBlockPositiveLayerInputV1,
    block_composition::complete_multi_block_report_matches_parent_for_test_v1,
    certify_canonical_positive_thickness_cycle_schedule_path_v1,
    certify_general_multi_face_cell_transport_v1, diagnose_block_union_completeness_v1,
    issue_common_articulation_block_composed_path_authority_v1,
    issue_common_articulation_block_composed_path_authority_with_control_v1,
    issue_common_articulation_continuous_layer_path_authority_with_control_v1,
    issue_common_articulation_pose_authority_v1,
    issue_complete_multi_block_positive_layer_authority_v1, issue_multi_block_closure_authority_v1,
    issue_multi_block_positive_layer_authority_v1,
};
use ori_core::{analyze_global_flat_foldability, analyze_local_flat_foldability};
use ori_domain::{
    CreasePattern, Edge, EdgeId, EdgeKind, FaceId, Paper, Point2, ProjectId, Vertex, VertexId,
};
use ori_foldability::{
    GlobalFlatFoldabilityInput, GlobalFlatFoldabilityLimits, LayerOrderSnapshot,
};
use ori_kinematics::{
    CanonicalCycleScheduleV1, CanonicalEdgeBlockLimitsV1, CanonicalHingeAngles,
    CycleScheduleLimitsV1, DyadicIntervalClosureLimitsV1, HalfAngleRationalEntryInputV1,
    HingeAngle, RationalCoefficientV1, TreeKinematicsLimits,
};
use ori_topology::{FaceExtractionInput, analyze_faces};

struct ClearanceFixtureV1 {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    pose: ClosedMaterialHingeGraphPose,
    decomposition: CanonicalMaterialEdgeBlockDecompositionV1,
    common_pose: CommonArticulationPoseAuthorityV1,
    schedule: CanonicalCycleScheduleV1,
    closure: DyadicMaterialHingeIntervalClosureCertificateV1,
    pairs: Vec<CommonArticulationCrossBlockFacePairV1>,
    positive: PositiveThicknessContinuousCertificateV1,
    source: Box<LayerOrderSnapshot>,
    paper_thickness_mm: f64,
}

impl ClearanceFixtureV1 {
    fn input<'a>(
        &'a self,
        submitted_cross_block_pairs: &'a [CommonArticulationCrossBlockFacePairV1],
        whole_parent_continuous: Option<PositiveThicknessContinuousCertificateV1>,
        limits: CommonArticulationClearanceLimitsV1,
    ) -> CommonArticulationClearanceInputV1<'a> {
        self.input_with(
            &self.geometry,
            &self.audit,
            &self.pose,
            &self.decomposition,
            &self.common_pose,
            &self.schedule,
            &self.closure,
            self.paper_thickness_mm,
            submitted_cross_block_pairs,
            whole_parent_continuous,
            limits,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn input_with<'a>(
        &'a self,
        geometry: &'a MaterialHingeGraphGeometry,
        audit: &'a MaterialHingeGraphAudit,
        pose: &'a ClosedMaterialHingeGraphPose,
        decomposition: &'a CanonicalMaterialEdgeBlockDecompositionV1,
        common_pose: &'a CommonArticulationPoseAuthorityV1,
        schedule: &'a CanonicalCycleScheduleV1,
        closure: &'a DyadicMaterialHingeIntervalClosureCertificateV1,
        paper_thickness_mm: f64,
        submitted_cross_block_pairs: &'a [CommonArticulationCrossBlockFacePairV1],
        whole_parent_continuous: Option<PositiveThicknessContinuousCertificateV1>,
        limits: CommonArticulationClearanceLimitsV1,
    ) -> CommonArticulationClearanceInputV1<'a> {
        CommonArticulationClearanceInputV1 {
            geometry,
            audit,
            pose,
            decomposition,
            common_pose,
            common_pose_limits: CommonArticulationPoseLimitsV1::default(),
            schedule,
            schedule_limits: CycleScheduleLimitsV1::default(),
            closure,
            paper_thickness_mm,
            submitted_cross_block_pairs,
            whole_parent_continuous,
            limits,
        }
    }
}

fn coefficient_v1(numerator: i64, denominator: u64) -> RationalCoefficientV1 {
    RationalCoefficientV1 {
        numerator,
        denominator,
    }
}

fn prepare_schedule_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    numerator_power_coefficients: Vec<RationalCoefficientV1>,
) -> (
    CanonicalCycleScheduleV1,
    DyadicMaterialHingeIntervalClosureCertificateV1,
) {
    prepare_schedule_with_domain_v1(
        geometry,
        audit,
        fixed_face,
        [coefficient_v1(0, 1), coefficient_v1(1, 1)],
        numerator_power_coefficients,
    )
}

fn prepare_schedule_with_domain_v1(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
    u_domain: [RationalCoefficientV1; 2],
    numerator_power_coefficients: Vec<RationalCoefficientV1>,
) -> (
    CanonicalCycleScheduleV1,
    DyadicMaterialHingeIntervalClosureCertificateV1,
) {
    let entries = geometry
        .hinges()
        .iter()
        .map(|hinge| HalfAngleRationalEntryInputV1 {
            edge: hinge.edge(),
            u_domain: u_domain.clone(),
            numerator_power_coefficients: numerator_power_coefficients.clone(),
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
    .expect("canonical strip schedule");
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
        .expect("canonical strip closure");
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
    pairs.sort_unstable_by(compare_pair_v1);
    pairs.dedup();
    pairs
}

fn prepare_strip_fixture_v1(block_count: usize) -> ClearanceFixtureV1 {
    assert!((2..=COMMON_ARTICULATION_CLEARANCE_MAX_BLOCKS_V1).contains(&block_count));
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
    .expect("strip topology");
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("strip geometry");
    let audit = MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
        .expect("strip audit");
    assert_eq!(geometry.face_ids().len(), face_count);
    assert_eq!(geometry.hinges().len(), block_count);
    let decomposition = geometry
        .decompose_canonical_edge_blocks_v1(
            &audit,
            CanonicalEdgeBlockLimitsV1 {
                max_blocks: COMMON_ARTICULATION_CLEARANCE_MAX_BLOCKS_V1,
                max_faces_per_block: 2,
                max_hinges_per_block: 2,
            },
        )
        .expect("canonical strip decomposition");
    assert_eq!(decomposition.blocks().len(), block_count);
    let fixed_face = if block_count == 2 {
        decomposition.articulation_faces()[0]
    } else {
        geometry.face_ids()[0]
    };
    let angles = CanonicalHingeAngles::new(
        geometry
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).expect("zero strip angle"))
            .collect(),
    )
    .expect("canonical strip angles");
    let pose = geometry
        .solve_closed(&audit, fixed_face, &angles, 0.0)
        .expect("closed strip pose");
    let common_pose = issue_common_articulation_pose_authority_v1(CommonArticulationPoseInputV1 {
        geometry: &geometry,
        pose: &pose,
        decomposition: &decomposition,
        paper_thickness_mm,
        limits: CommonArticulationPoseLimitsV1::default(),
    })
    .expect("common strip articulation pose");
    let (schedule, closure) = prepare_schedule_v1(
        &geometry,
        &audit,
        fixed_face,
        vec![coefficient_v1(0, 1), coefficient_v1(0, 1)],
    );
    let positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &geometry,
        &audit,
        fixed_face,
        &schedule,
        &closure,
        paper_thickness_mm,
        16,
    )
    .expect("whole-parent positive-thickness strip certificate");
    let local = analyze_local_flat_foldability(&paper, &pattern);
    let source = analyze_global_flat_foldability(
        GlobalFlatFoldabilityInput::current_with_geometry(
            namespace, &paper, &pattern, &topology, &local,
        ),
        GlobalFlatFoldabilityLimits::default(),
    )
    .expect("strip global flat foldability")
    .layer_order()
    .expect("strip source layer order")
    .clone();
    let pairs = independent_cross_block_pairs_v1(&decomposition);
    assert!(!pairs.is_empty());
    ClearanceFixtureV1 {
        geometry,
        audit,
        pose,
        decomposition,
        common_pose,
        schedule,
        closure,
        pairs,
        positive,
        source: Box::new(source),
        paper_thickness_mm,
    }
}

fn prepare_cactus_fixture_v1(block_count: usize) -> ClearanceFixtureV1 {
    assert!((2..=COMMON_ARTICULATION_CLEARANCE_MAX_BLOCKS_V1).contains(&block_count));
    let (pattern, paper, _) = match block_count {
        2 => {
            crate::miura_cactus_test_support::independent_three_by_three_miura_blocks_with_document(
            )
            .1
        }
        3 => crate::miura_cactus_test_support::three_three_by_three_miura_blocks_with_document().1,
        _ => crate::miura_cactus_test_support::miura_block_chain_with_document(block_count),
    };
    let namespace = ProjectId::new();
    let paper_thickness_mm = paper.thickness_mm;
    let topology = analyze_faces(FaceExtractionInput {
        identity_namespace: namespace,
        source_revision: 1,
        paper: &paper,
        pattern: &pattern,
    })
    .snapshot
    .expect("cactus topology");
    let geometry = MaterialHingeGraphGeometry::prepare(
        &pattern,
        &paper,
        &topology,
        TreeKinematicsLimits::default(),
    )
    .expect("cactus geometry");
    let audit = MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default())
        .expect("cactus audit");
    let decomposition = geometry
        .decompose_canonical_edge_blocks_v1(
            &audit,
            CanonicalEdgeBlockLimitsV1 {
                max_blocks: COMMON_ARTICULATION_CLEARANCE_MAX_BLOCKS_V1,
                ..CanonicalEdgeBlockLimitsV1::default()
            },
        )
        .expect("cactus decomposition");
    assert_eq!(decomposition.blocks().len(), block_count);
    let fixed_face = geometry.face_ids()[0];
    let angles = CanonicalHingeAngles::new(
        geometry
            .hinges()
            .iter()
            .map(|hinge| HingeAngle::new(hinge.edge(), 0.0).expect("zero cactus angle"))
            .collect(),
    )
    .expect("canonical cactus angles");
    let pose = geometry
        .solve_closed(&audit, fixed_face, &angles, 0.0)
        .expect("closed cactus pose");
    let common_pose = issue_common_articulation_pose_authority_v1(CommonArticulationPoseInputV1 {
        geometry: &geometry,
        pose: &pose,
        decomposition: &decomposition,
        paper_thickness_mm,
        limits: CommonArticulationPoseLimitsV1::default(),
    })
    .expect("common cactus articulation pose");
    let (schedule, closure) = prepare_schedule_v1(
        &geometry,
        &audit,
        fixed_face,
        vec![coefficient_v1(0, 1), coefficient_v1(0, 1)],
    );
    let positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
        &geometry,
        &audit,
        fixed_face,
        &schedule,
        &closure,
        paper_thickness_mm,
        16,
    )
    .expect("whole-parent positive-thickness cactus certificate");
    let local = analyze_local_flat_foldability(&paper, &pattern);
    let source = analyze_global_flat_foldability(
        GlobalFlatFoldabilityInput::current_with_geometry(
            namespace, &paper, &pattern, &topology, &local,
        ),
        GlobalFlatFoldabilityLimits::default(),
    )
    .expect("cactus global flat foldability")
    .layer_order()
    .expect("cactus source layer order")
    .clone();
    let pairs = independent_cross_block_pairs_v1(&decomposition);
    ClearanceFixtureV1 {
        geometry,
        audit,
        pose,
        decomposition,
        common_pose,
        schedule,
        closure,
        pairs,
        positive,
        source: Box::new(source),
        paper_thickness_mm,
    }
}

fn canonical_edge_partition_v1(
    decomposition: &CanonicalMaterialEdgeBlockDecompositionV1,
) -> Vec<Vec<EdgeId>> {
    decomposition
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

fn issue_fixture_clearance_v1(
    fixture: &ClearanceFixtureV1,
    limits: CommonArticulationClearanceLimitsV1,
) -> CommonArticulationClearancePrerequisiteV1 {
    let outcome = issue_common_articulation_clearance_prerequisite_v1(fixture.input(
        &fixture.pairs,
        Some(fixture.positive.clone()),
        limits,
    ))
    .expect("fixture clearance issuance");
    match outcome {
        CommonArticulationClearanceOutcomeV1::Certified(prerequisite) => *prerequisite,
        CommonArticulationClearanceOutcomeV1::Unsupported(_) => {
            panic!("fixture has a whole-parent positive certificate")
        }
    }
}

fn fixture_revalidation_input_v1(
    fixture: &ClearanceFixtureV1,
    limits: CommonArticulationClearanceLimitsV1,
) -> CommonArticulationClearanceRevalidationInputV1<'_> {
    CommonArticulationClearanceRevalidationInputV1 {
        geometry: &fixture.geometry,
        audit: &fixture.audit,
        pose: &fixture.pose,
        decomposition: &fixture.decomposition,
        common_pose: &fixture.common_pose,
        common_pose_limits: CommonArticulationPoseLimitsV1::default(),
        schedule: &fixture.schedule,
        schedule_limits: CycleScheduleLimitsV1::default(),
        closure: &fixture.closure,
        paper_thickness_mm: fixture.paper_thickness_mm,
        limits,
    }
}

fn restrict_layer_source_v1(source: &LayerOrderSnapshot, faces: &[FaceId]) -> LayerOrderSnapshot {
    let contains = |face: FaceId| faces.contains(&face);
    let mut restricted = source.clone();
    restricted
        .material_faces
        .retain(|face| contains(face.face_id));
    if let Some(global) = &mut restricted.global_bottom_to_top {
        global.retain(|face| contains(face.face_id));
    }
    restricted
        .folded_faces
        .retain(|face| contains(face.face.face_id));
    for cell in &mut restricted.overlap_cells {
        cell.covering_faces.retain(|face| contains(face.face_id));
        cell.bottom_to_top_faces.retain(|face| contains(*face));
    }
    restricted
        .overlap_cells
        .retain(|cell| !cell.bottom_to_top_faces.is_empty());
    restricted
        .face_pair_orders
        .retain(|pair| contains(pair.lower_face.face_id) && contains(pair.upper_face.face_id));
    restricted.reference_face = restricted
        .reference_face
        .filter(|face| contains(face.face_id))
        .or_else(|| restricted.material_faces.first().copied());
    restricted
}

struct FinalPathFixtureV1 {
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    pose: ClosedMaterialHingeGraphPose,
    decomposition: CanonicalMaterialEdgeBlockDecompositionV1,
    staged: CommonArticulationBlockComposedPathAuthorityV1,
    schedule: CanonicalCycleScheduleV1,
    closure: DyadicMaterialHingeIntervalClosureCertificateV1,
    complete: CompleteMultiBlockPositiveLayerAuthorityV1,
    block_sources: Vec<LayerOrderSnapshot>,
    target_angles: Vec<(EdgeId, f64)>,
    source: Box<LayerOrderSnapshot>,
    whole_parent_layer: GeneralMultiFaceCellTransportProofV1,
    paper_thickness_mm: f64,
    clearance_pair_count: usize,
    issuer_context: [u8; 32],
    articulation_layer_fingerprint: [u8; 32],
}

impl FinalPathFixtureV1 {
    fn issue(
        self,
        clearance_limits: CommonArticulationClearanceLimitsV1,
        replay_source: bool,
        source_override: Option<&LayerOrderSnapshot>,
        control: &CooperativeOperationControlV1<'_>,
    ) -> Result<
        CommonArticulationContinuousLayerPathAuthorityV1,
        CommonArticulationContinuousLayerPathErrorV1,
    > {
        let replayed_source = replay_source.then(|| (*self.source).clone());
        let source = source_override
            .or(replayed_source.as_ref())
            .unwrap_or(self.source.as_ref());
        let block_sources = self.block_sources.iter().collect::<Vec<_>>();
        issue_common_articulation_continuous_layer_path_authority_with_control_v1(
            CommonArticulationContinuousLayerPathInputV1 {
                geometry: &self.geometry,
                audit: &self.audit,
                pose: &self.pose,
                decomposition: &self.decomposition,
                staged: self.staged,
                common_pose_limits: CommonArticulationPoseLimitsV1::default(),
                schedule: &self.schedule,
                schedule_limits: CycleScheduleLimitsV1::default(),
                closure: &self.closure,
                paper_thickness_mm: self.paper_thickness_mm,
                clearance_limits,
                complete: self.complete,
                block_sources: &block_sources,
                issuer_context: self.issuer_context,
                articulation_layer_fingerprint: self.articulation_layer_fingerprint,
                target_angles: &self.target_angles,
                source,
                whole_parent_layer: self.whole_parent_layer,
            },
            control,
        )
    }

    fn issue_for_revalidation(self) -> IssuedFinalPathFixtureV1 {
        let FinalPathFixtureV1 {
            geometry,
            audit,
            pose,
            decomposition,
            staged,
            schedule,
            closure,
            complete,
            block_sources,
            target_angles,
            source,
            whole_parent_layer,
            paper_thickness_mm,
            clearance_pair_count: _,
            issuer_context,
            articulation_layer_fingerprint,
        } = self;
        let block_source_refs = block_sources.iter().collect::<Vec<_>>();
        let authority = issue_common_articulation_continuous_layer_path_authority_with_control_v1(
            CommonArticulationContinuousLayerPathInputV1 {
                geometry: &geometry,
                audit: &audit,
                pose: &pose,
                decomposition: &decomposition,
                staged,
                common_pose_limits: CommonArticulationPoseLimitsV1::default(),
                schedule: &schedule,
                schedule_limits: CycleScheduleLimitsV1::default(),
                closure: &closure,
                paper_thickness_mm,
                clearance_limits: CommonArticulationClearanceLimitsV1::default(),
                complete,
                block_sources: &block_source_refs,
                issuer_context,
                articulation_layer_fingerprint,
                target_angles: &target_angles,
                source: source.as_ref(),
                whole_parent_layer,
            },
            &CooperativeOperationControlV1::unbounded(),
        )
        .expect("issue final authority retained for revalidation");
        IssuedFinalPathFixtureV1 {
            authority,
            geometry,
            audit,
            pose,
            decomposition,
            schedule,
            closure,
            block_sources,
            target_angles,
            source,
            paper_thickness_mm,
            issuer_context,
            articulation_layer_fingerprint,
        }
    }
}

struct IssuedFinalPathFixtureV1 {
    authority: CommonArticulationContinuousLayerPathAuthorityV1,
    geometry: MaterialHingeGraphGeometry,
    audit: MaterialHingeGraphAudit,
    pose: ClosedMaterialHingeGraphPose,
    decomposition: CanonicalMaterialEdgeBlockDecompositionV1,
    schedule: CanonicalCycleScheduleV1,
    closure: DyadicMaterialHingeIntervalClosureCertificateV1,
    block_sources: Vec<LayerOrderSnapshot>,
    target_angles: Vec<(EdgeId, f64)>,
    source: Box<LayerOrderSnapshot>,
    paper_thickness_mm: f64,
    issuer_context: [u8; 32],
    articulation_layer_fingerprint: [u8; 32],
}

impl IssuedFinalPathFixtureV1 {
    fn input<'a>(
        &'a self,
        block_sources: &'a [&'a LayerOrderSnapshot],
    ) -> CommonArticulationContinuousLayerPathRevalidationInputV1<'a> {
        CommonArticulationContinuousLayerPathRevalidationInputV1 {
            geometry: &self.geometry,
            audit: &self.audit,
            pose: &self.pose,
            decomposition: &self.decomposition,
            common_pose_limits: CommonArticulationPoseLimitsV1::default(),
            schedule: &self.schedule,
            schedule_limits: CycleScheduleLimitsV1::default(),
            closure: &self.closure,
            paper_thickness_mm: self.paper_thickness_mm,
            clearance_limits: CommonArticulationClearanceLimitsV1::default(),
            block_sources,
            issuer_context: self.issuer_context,
            articulation_layer_fingerprint: self.articulation_layer_fingerprint,
            target_angles: &self.target_angles,
            source: self.source.as_ref(),
        }
    }
}

fn prepare_final_path_fixture_v1() -> FinalPathFixtureV1 {
    prepare_final_path_fixture_with_variants_v1(false, false, 2)
}

fn prepare_final_path_fixture_with_noncanonical_block_schedules_v1(
    noncanonical_block_schedules: bool,
) -> FinalPathFixtureV1 {
    prepare_final_path_fixture_with_variants_v1(noncanonical_block_schedules, false, 2)
}

fn prepare_final_path_fixture_with_variants_v1(
    noncanonical_block_schedules: bool,
    foreign_block_source: bool,
    block_count: usize,
) -> FinalPathFixtureV1 {
    let fixture = prepare_cactus_fixture_v1(block_count);
    prepare_final_path_fixture_from_clearance_v1(
        fixture,
        noncanonical_block_schedules,
        foreign_block_source,
    )
}

fn prepare_final_path_fixture_from_clearance_v1(
    fixture: ClearanceFixtureV1,
    noncanonical_block_schedules: bool,
    foreign_block_source: bool,
) -> FinalPathFixtureV1 {
    let clearance_pair_count = fixture.pairs.len();
    let clearance_limits = CommonArticulationClearanceLimitsV1::default();
    let clearance = issue_fixture_clearance_v1(&fixture, clearance_limits);
    let whole_parent_layer =
        certify_general_multi_face_cell_transport_v1(GeneralCellTransportInputV1 {
            geometry: &fixture.geometry,
            audit: &fixture.audit,
            source: &fixture.source,
            schedule: &fixture.schedule,
            closure: &fixture.closure,
            positive_continuous: &fixture.positive,
            paper_thickness_mm: fixture.paper_thickness_mm,
            tolerance: 1.0e-9,
            limits: GeneralCellTransportLimitsV1 {
                max_transitions: fixture.closure.leaves().len() + 1,
                max_cells: 1_000_000,
                max_layer_records: 1_000_000,
                max_boundary_samples: 1_000_000,
            },
        })
        .expect("whole-parent strip layer transport");
    let ClearanceFixtureV1 {
        geometry,
        audit,
        pose,
        decomposition,
        common_pose,
        schedule,
        closure,
        source,
        paper_thickness_mm,
        ..
    } = fixture;
    let staged = issue_common_articulation_block_composed_path_authority_v1(
        CommonArticulationBlockComposedPathInputV1 {
            geometry: &geometry,
            audit: &audit,
            pose: &pose,
            decomposition: &decomposition,
            common_pose,
            common_pose_limits: CommonArticulationPoseLimitsV1::default(),
            schedule: &schedule,
            schedule_limits: CycleScheduleLimitsV1::default(),
            closure: &closure,
            paper_thickness_mm,
            clearance,
            clearance_limits,
            blocks: canonical_edge_partition_v1(&decomposition),
        },
    )
    .expect("staged final-path prerequisite");

    let block_schedules = decomposition
        .blocks()
        .iter()
        .map(|block| {
            let block_fixed_face = block
                .geometry()
                .face_ids()
                .iter()
                .copied()
                .find(|face| decomposition.articulation_faces().contains(face))
                .expect("block articulation face");
            let block_schedule = if noncanonical_block_schedules {
                CanonicalCycleScheduleV1::prepare_half_angle_rational(
                    block.geometry(),
                    block.audit(),
                    block_fixed_face,
                    block
                        .geometry()
                        .hinges()
                        .iter()
                        .map(|hinge| HalfAngleRationalEntryInputV1 {
                            edge: hinge.edge(),
                            u_domain: [coefficient_v1(0, 1), coefficient_v1(1, 1)],
                            numerator_power_coefficients: vec![
                                coefficient_v1(0, 1),
                                coefficient_v1(0, 1),
                            ],
                            denominator_power_coefficients: vec![coefficient_v1(63, 1)],
                        })
                        .collect(),
                    CycleScheduleLimitsV1::default(),
                )
                .expect("same-path noncanonical block schedule")
            } else {
                schedule
                    .restrict_to_edge_block_with_fixed_face_v1(
                        &geometry,
                        &audit,
                        block.geometry(),
                        block.audit(),
                        block_fixed_face,
                    )
                    .expect("exact full-schedule block restriction")
            };
            let block_closure = block
                .geometry()
                .prove_dyadic_schedule_closure_v1(
                    block.audit(),
                    block_fixed_face,
                    &block_schedule,
                    1.0e-9,
                    DyadicIntervalClosureLimitsV1 {
                        max_depth: 8,
                        max_leaves: 256,
                        max_work: 1_000_000,
                        schedule_limits: CycleScheduleLimitsV1::default(),
                    },
                )
                .expect("restricted block closure");
            (block_schedule, block_closure)
        })
        .collect::<Vec<_>>();
    let mut source_records = decomposition
        .blocks()
        .iter()
        .map(|block| {
            (
                block
                    .geometry()
                    .hinges()
                    .iter()
                    .map(|hinge| hinge.edge().canonical_bytes())
                    .min()
                    .expect("non-empty canonical block"),
                restrict_layer_source_v1(&source, block.geometry().face_ids()),
            )
        })
        .collect::<Vec<_>>();
    source_records.sort_unstable_by_key(|(edge, _)| *edge);
    let canonical_block_keys = source_records
        .iter()
        .map(|(edge, _)| *edge)
        .collect::<Vec<_>>();
    let mut block_sources = source_records
        .into_iter()
        .map(|(_, source)| source)
        .collect::<Vec<_>>();
    if foreign_block_source {
        block_sources[0].material_faces.reverse();
    }
    let issuer_context = [0x61; 32];
    let articulation_layer_fingerprint = [0x62; 32];
    let parent = issue_multi_block_closure_authority_v1(
        decomposition
            .blocks()
            .iter()
            .zip(&block_schedules)
            .map(|(block, (schedule, closure))| MultiBlockClosureInputV1 {
                geometry: block.geometry(),
                audit: block.audit(),
                schedule,
                closure,
            })
            .collect(),
        paper_thickness_mm,
        issuer_context,
    )
    .expect("final-path block closure");
    let block_proofs = decomposition
        .blocks()
        .iter()
        .zip(&block_schedules)
        .map(|(block, (block_schedule, block_closure))| {
            let key = block
                .geometry()
                .hinges()
                .iter()
                .map(|hinge| hinge.edge().canonical_bytes())
                .min()
                .expect("non-empty canonical block");
            let block_source = &block_sources[canonical_block_keys
                .binary_search(&key)
                .expect("canonical block source")];
            let positive = certify_canonical_positive_thickness_cycle_schedule_path_v1(
                block.geometry(),
                block.audit(),
                block_closure.fixed_face(),
                block_schedule,
                block_closure,
                paper_thickness_mm,
                16,
            )
            .expect("final-path block positive path");
            let layer = certify_general_multi_face_cell_transport_v1(GeneralCellTransportInputV1 {
                geometry: block.geometry(),
                audit: block.audit(),
                source: block_source,
                schedule: block_schedule,
                closure: block_closure,
                positive_continuous: &positive,
                paper_thickness_mm,
                tolerance: 1.0e-9,
                limits: GeneralCellTransportLimitsV1 {
                    max_transitions: block_closure.leaves().len() + 1,
                    max_cells: 1_000_000,
                    max_layer_records: 1_000_000,
                    max_boundary_samples: 1_000_000,
                },
            })
            .expect("final-path block layer transport");
            (positive, layer)
        })
        .collect::<Vec<_>>();
    let parent = issue_multi_block_positive_layer_authority_v1(
        parent,
        decomposition
            .blocks()
            .iter()
            .zip(block_proofs)
            .map(|(block, (positive, layer))| {
                let key = block
                    .geometry()
                    .hinges()
                    .iter()
                    .map(|hinge| hinge.edge().canonical_bytes())
                    .min()
                    .expect("non-empty canonical block");
                let block_source = &block_sources[canonical_block_keys
                    .binary_search(&key)
                    .expect("canonical block source")];
                MultiBlockPositiveLayerInputV1 {
                    geometry: block.geometry(),
                    source: block_source,
                    positive,
                    layer,
                }
            })
            .collect(),
        articulation_layer_fingerprint,
    )
    .expect("final-path block positive-layer authority");
    // `BlockUnionCompletenessInputV1` borrows each hinge slice, so keep the
    // owned canonical slices alive across report issuance.
    let block_hinges = decomposition
        .blocks()
        .iter()
        .map(|block| {
            block
                .geometry()
                .hinges()
                .iter()
                .map(|hinge| hinge.edge())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let report = diagnose_block_union_completeness_v1(
        &geometry,
        &decomposition
            .blocks()
            .iter()
            .zip(&block_hinges)
            .map(|(block, hinges)| BlockUnionCompletenessInputV1 {
                faces: block.geometry().face_ids(),
                hinges,
            })
            .collect::<Vec<_>>(),
    )
    .expect("final-path completeness report");
    assert!(report.exact_live_union_observed());
    let block_source_refs = block_sources.iter().collect::<Vec<_>>();
    let target_angles = block_schedules
        .iter()
        .flat_map(|(block_schedule, _)| {
            block_schedule
                .evaluate(1.0)
                .expect("block target")
                .as_slice()
                .iter()
                .map(|angle| (angle.edge(), angle.angle_degrees()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert!(
        complete_multi_block_report_matches_parent_for_test_v1(&geometry, &report, &parent),
        "complete report/parent mismatch"
    );
    assert!(
        parent.revalidates_v1(
            &block_source_refs,
            paper_thickness_mm,
            issuer_context,
            articulation_layer_fingerprint,
        ),
        "positive-layer parent mismatch"
    );
    assert!(
        parent.target_angles_match_v1(&target_angles),
        "target-angle mismatch"
    );
    let complete = issue_complete_multi_block_positive_layer_authority_v1(
        &geometry,
        report,
        parent,
        &block_source_refs,
        paper_thickness_mm,
        issuer_context,
        articulation_layer_fingerprint,
        &target_angles,
    )
    .expect("final-path complete block authority");
    FinalPathFixtureV1 {
        geometry,
        audit,
        pose,
        decomposition,
        staged,
        schedule,
        closure,
        complete,
        block_sources,
        target_angles,
        source,
        whole_parent_layer,
        paper_thickness_mm,
        clearance_pair_count,
        issuer_context,
        articulation_layer_fingerprint,
    }
}

#[test]
fn unordered_pairs_are_canonical_and_self_pairs_are_rejected_v1() {
    let first = FaceId::new();
    let second = FaceId::new();
    let forward =
        CommonArticulationCrossBlockFacePairV1::new(first, second).expect("distinct pair");
    let reverse = CommonArticulationCrossBlockFacePairV1::new(second, first).expect("reverse pair");
    assert_eq!(forward, reverse);
    assert!(
        forward.first().canonical_bytes() < forward.second().canonical_bytes(),
        "the stored pair order is canonical"
    );
    assert!(CommonArticulationCrossBlockFacePairV1::new(first, first).is_none());
}

#[test]
fn submitted_pair_registry_rejects_missing_extra_and_duplicate_but_not_order_v1() {
    let [a, b, c, d] = std::array::from_fn(|_| FaceId::new());
    let mut expected = [
        CommonArticulationCrossBlockFacePairV1::new(a, c).expect("a/c"),
        CommonArticulationCrossBlockFacePairV1::new(a, d).expect("a/d"),
        CommonArticulationCrossBlockFacePairV1::new(b, c).expect("b/c"),
    ];
    expected.sort_unstable_by(compare_pair_v1);
    let mut checkpoint = || Ok(());
    let reversed = [expected[2], expected[0], expected[1]];
    validate_submitted_pairs_v1(&reversed, &expected, &mut checkpoint)
        .expect("caller list order is not semantic");

    assert_eq!(
        validate_submitted_pairs_v1(&expected[..2], &expected, &mut checkpoint),
        Err(
            CommonArticulationClearanceErrorV1::CrossBlockPairCoverageMismatch {
                expected: 3,
                actual: 2,
            }
        )
    );
    let extra_pair =
        CommonArticulationCrossBlockFacePairV1::new(c, d).expect("extra distinct pair");
    let extra = [expected[0], expected[1], expected[2], extra_pair];
    assert_eq!(
        validate_submitted_pairs_v1(&extra, &expected, &mut checkpoint),
        Err(
            CommonArticulationClearanceErrorV1::CrossBlockPairCoverageMismatch {
                expected: 3,
                actual: 4,
            }
        )
    );
    let duplicate = [expected[0], expected[1], expected[1], expected[2]];
    assert_eq!(
        validate_submitted_pairs_v1(&duplicate, &expected, &mut checkpoint),
        Err(CommonArticulationClearanceErrorV1::DuplicateCrossBlockPair)
    );
}

#[test]
fn hard_limits_cannot_be_relaxed_v1() {
    let defaults = CommonArticulationClearanceLimitsV1::default();
    for excessive in [
        CommonArticulationClearanceLimitsV1 {
            max_blocks: defaults.max_blocks + 1,
            ..defaults
        },
        CommonArticulationClearanceLimitsV1 {
            max_faces: defaults.max_faces + 1,
            ..defaults
        },
        CommonArticulationClearanceLimitsV1 {
            max_cross_block_pairs: defaults.max_cross_block_pairs + 1,
            ..defaults
        },
        CommonArticulationClearanceLimitsV1 {
            max_pair_candidates: defaults.max_pair_candidates + 1,
            ..defaults
        },
        CommonArticulationClearanceLimitsV1 {
            max_work: defaults.max_work + 1,
            ..defaults
        },
        CommonArticulationClearanceLimitsV1 {
            max_storage_bytes: defaults.max_storage_bytes + 1,
            ..defaults
        },
    ] {
        assert_eq!(
            validate_limits_v1(excessive),
            Err(CommonArticulationClearanceErrorV1::ResourceLimit)
        );
    }
}

#[test]
fn cross_block_clearance_proves_three_five_and_eight_block_positive_thickness_v1() {
    for block_count in [3, 5, 8] {
        let fixture = prepare_strip_fixture_v1(block_count);
        let outcome = issue_common_articulation_clearance_prerequisite_v1(fixture.input(
            &fixture.pairs,
            Some(fixture.positive.clone()),
            CommonArticulationClearanceLimitsV1::default(),
        ))
        .expect("whole-parent cross-block clearance");
        assert!(outcome.is_certified());
        assert!(outcome.as_gap().is_none());
        let authority = outcome.as_certified().expect("certified prerequisite");
        assert_eq!(
            authority.model_id(),
            COMMON_ARTICULATION_CLEARANCE_PREREQUISITE_MODEL_ID_V1
        );
        assert!(authority.is_for_pose_v1(&fixture.geometry, &fixture.pose));
        assert_eq!(authority.cross_block_pairs_v1(), fixture.pairs);
        assert_eq!(
            authority.paper_thickness_mm_v1().to_bits(),
            fixture.paper_thickness_mm.to_bits()
        );
        assert_eq!(
            authority.common_pose_binding_fingerprint_v1(),
            fixture.common_pose.binding_fingerprint_v1()
        );
        assert_eq!(
            authority.schedule_binding_fingerprint_v1(),
            fixture.schedule.certificate_binding_fingerprint_v2()
        );
        assert_eq!(
            authority.closure_binding_fingerprint_v1(),
            fixture.closure.partition_binding_fingerprint_v2()
        );
        assert_ne!(authority.binding_fingerprint_v1(), [0; 32]);
        assert!(authority.logical_work_v1() > 0);
        assert!(authority.storage_bytes_upper_bound_v1() > 0);
        assert!(authority.cross_block_open_interval_clearance_proven_v1());
        assert!(!authority.authorizes_continuous_motion());
        assert!(!authority.authorizes_collision_clearance());
        assert!(!authority.authorizes_project_mutation());
        assert!(!authority.authorizes_apply());
        assert!(!authority.authorizes_viewer());
    }
}

#[test]
fn sample_per_block_and_aabb_substitutes_remain_an_unsupported_gap_v1() {
    let fixture = prepare_strip_fixture_v1(3);
    let outcome = issue_common_articulation_clearance_prerequisite_v1(fixture.input(
        &fixture.pairs,
        None,
        CommonArticulationClearanceLimitsV1::default(),
    ))
    .expect("typed missing-whole-parent gap");
    assert!(!outcome.is_certified());
    assert!(outcome.as_certified().is_none());
    let gap = outcome.as_gap().expect("unsupported diagnostic");
    assert_eq!(
        gap.model_id(),
        COMMON_ARTICULATION_CLEARANCE_GAP_MODEL_ID_V1
    );
    assert_eq!(
        gap.reason(),
        CommonArticulationClearanceUnsupportedReasonV1::WholeParentOpenIntervalProofUnavailable
    );
    assert_eq!(gap.cross_block_pairs_v1(), fixture.pairs);
    assert_eq!(
        gap.paper_thickness_mm_v1().to_bits(),
        fixture.paper_thickness_mm.to_bits()
    );
    assert_eq!(
        gap.common_pose_binding_fingerprint_v1(),
        fixture.common_pose.binding_fingerprint_v1()
    );
    assert_eq!(
        gap.schedule_binding_fingerprint_v1(),
        fixture.schedule.certificate_binding_fingerprint_v2()
    );
    assert_eq!(
        gap.closure_binding_fingerprint_v1(),
        fixture.closure.partition_binding_fingerprint_v2()
    );
    assert!(gap.logical_work_v1() > 0);
    assert!(gap.storage_bytes_upper_bound_v1() > 0);
    assert!(!gap.endpoint_observations_are_authority_v1());
    assert!(!gap.sampled_poses_are_authority_v1());
    assert!(!gap.broad_phase_aabbs_are_authority_v1());
    assert!(!gap.per_block_certificates_are_cross_block_authority_v1());
    assert!(!gap.authorizes_continuous_motion());
    assert!(!gap.authorizes_collision_clearance());
    assert!(!gap.authorizes_project_mutation());
    assert!(!gap.authorizes_apply());
    assert!(!gap.authorizes_viewer());
}

#[test]
fn exact_bindings_reject_foreign_geometry_schedule_closure_common_pose_and_thickness_v1() {
    let fixture = prepare_strip_fixture_v1(3);
    let foreign = prepare_strip_fixture_v1(3);
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_v1(fixture.input_with(
            &foreign.geometry,
            &fixture.audit,
            &fixture.pose,
            &fixture.decomposition,
            &fixture.common_pose,
            &fixture.schedule,
            &fixture.closure,
            fixture.paper_thickness_mm,
            &fixture.pairs,
            Some(fixture.positive.clone()),
            CommonArticulationClearanceLimitsV1::default(),
        ))
        .expect_err("foreign geometry"),
        CommonArticulationClearanceErrorV1::InvalidInput
    );
    assert!(matches!(
        issue_common_articulation_clearance_prerequisite_v1(fixture.input_with(
            &fixture.geometry,
            &fixture.audit,
            &fixture.pose,
            &fixture.decomposition,
            &foreign.common_pose,
            &fixture.schedule,
            &fixture.closure,
            fixture.paper_thickness_mm,
            &fixture.pairs,
            Some(fixture.positive.clone()),
            CommonArticulationClearanceLimitsV1::default(),
        )),
        Err(CommonArticulationClearanceErrorV1::CommonPose(_))
    ));

    let (foreign_schedule, foreign_closure) = prepare_schedule_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.pose.fixed_face(),
        vec![coefficient_v1(0, 1), coefficient_v1(1, 1)],
    );
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_v1(fixture.input_with(
            &fixture.geometry,
            &fixture.audit,
            &fixture.pose,
            &fixture.decomposition,
            &fixture.common_pose,
            &foreign_schedule,
            &foreign_closure,
            fixture.paper_thickness_mm,
            &fixture.pairs,
            Some(fixture.positive.clone()),
            CommonArticulationClearanceLimitsV1::default(),
        ))
        .expect_err("foreign schedule"),
        CommonArticulationClearanceErrorV1::WholeParentContinuousProofMismatch
    );
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_v1(fixture.input_with(
            &fixture.geometry,
            &fixture.audit,
            &fixture.pose,
            &fixture.decomposition,
            &fixture.common_pose,
            &fixture.schedule,
            &foreign_closure,
            fixture.paper_thickness_mm,
            &fixture.pairs,
            Some(fixture.positive.clone()),
            CommonArticulationClearanceLimitsV1::default(),
        ))
        .expect_err("foreign closure"),
        CommonArticulationClearanceErrorV1::PathBindingMismatch
    );
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_v1(fixture.input(
            &fixture.pairs,
            Some(foreign.positive.clone()),
            CommonArticulationClearanceLimitsV1::default(),
        ))
        .expect_err("foreign whole-parent certificate"),
        CommonArticulationClearanceErrorV1::WholeParentContinuousProofMismatch
    );

    let one_ulp_thicker = f64::from_bits(fixture.paper_thickness_mm.to_bits() + 1);
    assert!(matches!(
        issue_common_articulation_clearance_prerequisite_v1(fixture.input_with(
            &fixture.geometry,
            &fixture.audit,
            &fixture.pose,
            &fixture.decomposition,
            &fixture.common_pose,
            &fixture.schedule,
            &fixture.closure,
            one_ulp_thicker,
            &fixture.pairs,
            Some(fixture.positive.clone()),
            CommonArticulationClearanceLimitsV1::default(),
        )),
        Err(CommonArticulationClearanceErrorV1::CommonPose(_))
    ));
}

#[test]
fn canonical_source_pose_requires_exact_schedule_zero_bits_v1() {
    let fixture = prepare_strip_fixture_v1(3);
    let (foreign_source_schedule, foreign_source_closure) = prepare_schedule_v1(
        &fixture.geometry,
        &fixture.audit,
        fixture.pose.fixed_face(),
        vec![coefficient_v1(1, 64), coefficient_v1(0, 1)],
    );
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_v1(fixture.input_with(
            &fixture.geometry,
            &fixture.audit,
            &fixture.pose,
            &fixture.decomposition,
            &fixture.common_pose,
            &foreign_source_schedule,
            &foreign_source_closure,
            fixture.paper_thickness_mm,
            &fixture.pairs,
            None,
            CommonArticulationClearanceLimitsV1::default(),
        ))
        .expect_err("foreign source angle bits"),
        CommonArticulationClearanceErrorV1::PathSourcePoseMismatch
    );
}

#[test]
fn cartesian_pair_coverage_is_complete_order_independent_and_fail_closed_v1() {
    let fixture = prepare_strip_fixture_v1(3);
    assert_eq!(
        fixture.pairs,
        independent_cross_block_pairs_v1(&fixture.decomposition)
    );
    let baseline = issue_common_articulation_clearance_prerequisite_v1(fixture.input(
        &fixture.pairs,
        Some(fixture.positive.clone()),
        CommonArticulationClearanceLimitsV1::default(),
    ))
    .expect("baseline pair coverage");
    let baseline_binding = baseline
        .as_certified()
        .expect("baseline authority")
        .binding_fingerprint_v1();

    let mut reversed = fixture.pairs.clone();
    reversed.reverse();
    let reordered = issue_common_articulation_clearance_prerequisite_v1(fixture.input(
        &reversed,
        Some(fixture.positive.clone()),
        CommonArticulationClearanceLimitsV1::default(),
    ))
    .expect("pair order is not semantic");
    assert_eq!(
        reordered
            .as_certified()
            .expect("reordered authority")
            .binding_fingerprint_v1(),
        baseline_binding
    );

    let missing = &fixture.pairs[..fixture.pairs.len() - 1];
    assert!(matches!(
        issue_common_articulation_clearance_prerequisite_v1(fixture.input(
            missing,
            Some(fixture.positive.clone()),
            CommonArticulationClearanceLimitsV1::default(),
        )),
        Err(
            CommonArticulationClearanceErrorV1::CrossBlockPairCoverageMismatch {
                expected,
                actual
            }
        ) if expected == fixture.pairs.len() && actual == missing.len()
    ));
    let mut extra = fixture.pairs.clone();
    extra.push(
        CommonArticulationCrossBlockFacePairV1::new(fixture.geometry.face_ids()[0], FaceId::new())
            .expect("foreign extra pair"),
    );
    assert!(matches!(
        issue_common_articulation_clearance_prerequisite_v1(fixture.input(
            &extra,
            Some(fixture.positive.clone()),
            CommonArticulationClearanceLimitsV1::default(),
        )),
        Err(
            CommonArticulationClearanceErrorV1::CrossBlockPairCoverageMismatch {
                expected,
                actual
            }
        ) if expected == fixture.pairs.len() && actual == extra.len()
    ));
    let mut duplicate = fixture.pairs.clone();
    duplicate.push(fixture.pairs[0]);
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_v1(fixture.input(
            &duplicate,
            Some(fixture.positive.clone()),
            CommonArticulationClearanceLimitsV1::default(),
        ))
        .expect_err("duplicate submitted pair"),
        CommonArticulationClearanceErrorV1::DuplicateCrossBlockPair
    );
}

#[test]
fn exact_resource_envelope_passes_and_every_one_short_limit_fails_closed_v1() {
    let fixture = prepare_strip_fixture_v1(3);
    let envelope = resource_envelope_v1(
        &fixture.decomposition,
        fixture.geometry.face_ids().len(),
        fixture.geometry.hinges().len(),
        fixture.pairs.len(),
        fixture.common_pose.logical_work_v1(),
    )
    .expect("exact resource envelope");
    let exact = CommonArticulationClearanceLimitsV1 {
        max_blocks: fixture.decomposition.blocks().len(),
        max_faces: fixture.geometry.face_ids().len(),
        max_cross_block_pairs: fixture.pairs.len(),
        max_pair_candidates: envelope.raw_pair_candidates,
        max_work: envelope.logical_work,
        max_storage_bytes: envelope.storage_bytes_upper_bound,
    };
    issue_common_articulation_clearance_prerequisite_v1(fixture.input(
        &fixture.pairs,
        Some(fixture.positive.clone()),
        exact,
    ))
    .expect("exact limits");

    for one_short in [
        CommonArticulationClearanceLimitsV1 {
            max_blocks: exact.max_blocks - 1,
            ..exact
        },
        CommonArticulationClearanceLimitsV1 {
            max_faces: exact.max_faces - 1,
            ..exact
        },
        CommonArticulationClearanceLimitsV1 {
            max_cross_block_pairs: exact.max_cross_block_pairs - 1,
            ..exact
        },
        CommonArticulationClearanceLimitsV1 {
            max_pair_candidates: exact.max_pair_candidates - 1,
            ..exact
        },
        CommonArticulationClearanceLimitsV1 {
            max_work: exact.max_work - 1,
            ..exact
        },
        CommonArticulationClearanceLimitsV1 {
            max_storage_bytes: exact.max_storage_bytes - 1,
            ..exact
        },
    ] {
        assert_eq!(
            issue_common_articulation_clearance_prerequisite_v1(fixture.input(
                &fixture.pairs,
                Some(fixture.positive.clone()),
                one_short,
            ))
            .expect_err("one-short clearance limit"),
            CommonArticulationClearanceErrorV1::ResourceLimit
        );
    }
}

#[test]
fn cooperative_stops_at_entry_loop_and_prepublish_return_no_authority_v1() {
    let fixture = prepare_strip_fixture_v1(3);
    let mut checkpoints = 0usize;
    issue_common_articulation_clearance_prerequisite_with_checkpoint_v1(
        fixture.input(
            &fixture.pairs,
            Some(fixture.positive.clone()),
            CommonArticulationClearanceLimitsV1::default(),
        ),
        &mut || {
            checkpoints += 1;
            Ok(())
        },
    )
    .expect("count clearance checkpoints");
    assert!(checkpoints > 4);

    for stop_at in [1, checkpoints / 2, checkpoints] {
        for expected in [
            CommonArticulationClearanceErrorV1::Cancelled,
            CommonArticulationClearanceErrorV1::DeadlineExceeded,
        ] {
            let mut observed = 0usize;
            assert_eq!(
                issue_common_articulation_clearance_prerequisite_with_checkpoint_v1(
                    fixture.input(
                        &fixture.pairs,
                        Some(fixture.positive.clone()),
                        CommonArticulationClearanceLimitsV1::default(),
                    ),
                    &mut || {
                        observed += 1;
                        if observed == stop_at {
                            Err(expected)
                        } else {
                            Ok(())
                        }
                    },
                )
                .expect_err("typed checkpoint stop"),
                expected
            );
        }
    }

    let cancelled = AtomicBool::new(true);
    let active = AtomicBool::new(false);
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_with_control_v1(
            fixture.input(
                &fixture.pairs,
                Some(fixture.positive.clone()),
                CommonArticulationClearanceLimitsV1::default(),
            ),
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        )
        .expect_err("entry cancellation"),
        CommonArticulationClearanceErrorV1::Cancelled
    );
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_with_control_v1(
            fixture.input(
                &fixture.pairs,
                Some(fixture.positive.clone()),
                CommonArticulationClearanceLimitsV1::default(),
            ),
            &CooperativeOperationControlV1::new(Some(&active), Instant::now()),
        )
        .expect_err("entry deadline"),
        CommonArticulationClearanceErrorV1::DeadlineExceeded
    );
    let generation = AtomicU64::new(12);
    assert_eq!(
        issue_common_articulation_clearance_prerequisite_with_control_v1(
            fixture.input(
                &fixture.pairs,
                Some(fixture.positive.clone()),
                CommonArticulationClearanceLimitsV1::default(),
            ),
            &CooperativeOperationControlV1::new_with_generation(
                Some(&active),
                &generation,
                11,
                Instant::now() + Duration::from_secs(1),
            ),
        )
        .expect_err("generation ABA cancellation"),
        CommonArticulationClearanceErrorV1::Cancelled
    );
    generation.store(11, Ordering::Release);
    issue_common_articulation_clearance_prerequisite_with_control_v1(
        fixture.input(
            &fixture.pairs,
            Some(fixture.positive.clone()),
            CommonArticulationClearanceLimitsV1::default(),
        ),
        &CooperativeOperationControlV1::new_with_generation(
            Some(&active),
            &generation,
            11,
            Instant::now() + Duration::from_secs(1),
        ),
    )
    .expect("current generation");
}

#[test]
fn clearance_prerequisite_exact_revalidation_and_one_short_limit_v1() {
    let fixture = prepare_strip_fixture_v1(3);
    let limits = CommonArticulationClearanceLimitsV1::default();
    let prerequisite = issue_fixture_clearance_v1(&fixture, limits);
    assert_eq!(
        prerequisite.revalidate_v1(fixture_revalidation_input_v1(&fixture, limits)),
        Ok(())
    );
    assert!(
        prerequisite
            .revalidate_v1(CommonArticulationClearanceRevalidationInputV1 {
                paper_thickness_mm: f64::from_bits(fixture.paper_thickness_mm.to_bits() + 1),
                ..fixture_revalidation_input_v1(&fixture, limits)
            })
            .is_err()
    );
    assert_eq!(
        prerequisite.revalidate_v1(fixture_revalidation_input_v1(
            &fixture,
            CommonArticulationClearanceLimitsV1 {
                max_cross_block_pairs: fixture.pairs.len() - 1,
                ..limits
            },
        )),
        Err(CommonArticulationClearanceErrorV1::ResourceLimit)
    );
}

#[test]
fn positive_three_block_staged_composition_moves_exact_prerequisites_v1() {
    let fixture = prepare_strip_fixture_v1(3);
    let blocks = canonical_edge_partition_v1(&fixture.decomposition);
    let clearance_limits = CommonArticulationClearanceLimitsV1::default();
    let clearance = issue_fixture_clearance_v1(&fixture, clearance_limits);
    let common_pose_binding = fixture.common_pose.binding_fingerprint_v1();
    let clearance_binding = clearance.binding_fingerprint_v1();
    let authority = issue_common_articulation_block_composed_path_authority_v1(
        CommonArticulationBlockComposedPathInputV1 {
            geometry: &fixture.geometry,
            audit: &fixture.audit,
            pose: &fixture.pose,
            decomposition: &fixture.decomposition,
            common_pose: fixture.common_pose,
            common_pose_limits: CommonArticulationPoseLimitsV1::default(),
            schedule: &fixture.schedule,
            schedule_limits: CycleScheduleLimitsV1::default(),
            closure: &fixture.closure,
            paper_thickness_mm: fixture.paper_thickness_mm,
            clearance,
            clearance_limits,
            blocks,
        },
    )
    .expect("positive three-block staged composition");
    assert_eq!(
        authority.model_id(),
        COMMON_ARTICULATION_BLOCK_COMPOSED_PATH_MODEL_ID_V1
    );
    assert_eq!(authority.block_count_v1(), 3);
    assert_eq!(
        authority.common_pose_binding_fingerprint_v1(),
        common_pose_binding
    );
    assert_eq!(
        authority.clearance_binding_fingerprint_v1(),
        clearance_binding
    );
    assert_ne!(authority.binding_fingerprint_v1(), [0; 32]);
    assert!(!authority.authorizes_continuous_motion());
    assert!(!authority.authorizes_collision_clearance());
    assert!(!authority.authorizes_project_mutation());
    assert!(!authority.authorizes_apply());
    assert!(!authority.authorizes_viewer());
}

#[test]
fn staged_composition_rejects_foreign_and_replayed_pose_authorities_v1() {
    let source = prepare_strip_fixture_v1(3);
    let clearance_limits = CommonArticulationClearanceLimitsV1::default();
    let clearance = issue_fixture_clearance_v1(&source, clearance_limits);
    let foreign = prepare_strip_fixture_v1(3);
    assert!(matches!(
        issue_common_articulation_block_composed_path_authority_v1(
            CommonArticulationBlockComposedPathInputV1 {
                geometry: &foreign.geometry,
                audit: &foreign.audit,
                pose: &foreign.pose,
                decomposition: &foreign.decomposition,
                common_pose: source.common_pose,
                common_pose_limits: CommonArticulationPoseLimitsV1::default(),
                schedule: &foreign.schedule,
                schedule_limits: CycleScheduleLimitsV1::default(),
                closure: &foreign.closure,
                paper_thickness_mm: foreign.paper_thickness_mm,
                clearance,
                clearance_limits,
                blocks: canonical_edge_partition_v1(&foreign.decomposition),
            },
        ),
        Err(CommonArticulationBlockComposedPathErrorV1::CommonPose(_))
    ));

    let fixture = prepare_strip_fixture_v1(3);
    let clearance = issue_fixture_clearance_v1(&fixture, clearance_limits);
    let replayed_pose = fixture
        .geometry
        .solve_closed(
            &fixture.audit,
            fixture.pose.fixed_face(),
            fixture.pose.hinge_angles(),
            0.0,
        )
        .expect("same-value independently issued pose");
    assert!(matches!(
        issue_common_articulation_block_composed_path_authority_v1(
            CommonArticulationBlockComposedPathInputV1 {
                geometry: &fixture.geometry,
                audit: &fixture.audit,
                pose: &replayed_pose,
                decomposition: &fixture.decomposition,
                common_pose: fixture.common_pose,
                common_pose_limits: CommonArticulationPoseLimitsV1::default(),
                schedule: &fixture.schedule,
                schedule_limits: CycleScheduleLimitsV1::default(),
                closure: &fixture.closure,
                paper_thickness_mm: fixture.paper_thickness_mm,
                clearance,
                clearance_limits,
                blocks: canonical_edge_partition_v1(&fixture.decomposition),
            },
        ),
        Err(CommonArticulationBlockComposedPathErrorV1::CommonPose(_))
    ));
}

#[test]
fn staged_composition_rejects_partition_and_one_short_clearance_v1() {
    let fixture = prepare_strip_fixture_v1(3);
    let clearance_limits = CommonArticulationClearanceLimitsV1::default();
    let clearance = issue_fixture_clearance_v1(&fixture, clearance_limits);
    let mut missing_block = canonical_edge_partition_v1(&fixture.decomposition);
    missing_block.pop();
    assert_eq!(
        issue_common_articulation_block_composed_path_authority_v1(
            CommonArticulationBlockComposedPathInputV1 {
                geometry: &fixture.geometry,
                audit: &fixture.audit,
                pose: &fixture.pose,
                decomposition: &fixture.decomposition,
                common_pose: fixture.common_pose,
                common_pose_limits: CommonArticulationPoseLimitsV1::default(),
                schedule: &fixture.schedule,
                schedule_limits: CycleScheduleLimitsV1::default(),
                closure: &fixture.closure,
                paper_thickness_mm: fixture.paper_thickness_mm,
                clearance,
                clearance_limits,
                blocks: missing_block,
            },
        )
        .unwrap_err(),
        CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch
    );

    let fixture = prepare_strip_fixture_v1(3);
    let clearance = issue_fixture_clearance_v1(&fixture, clearance_limits);
    let one_short = CommonArticulationClearanceLimitsV1 {
        max_cross_block_pairs: fixture.pairs.len() - 1,
        ..clearance_limits
    };
    assert_eq!(
        issue_common_articulation_block_composed_path_authority_v1(
            CommonArticulationBlockComposedPathInputV1 {
                geometry: &fixture.geometry,
                audit: &fixture.audit,
                pose: &fixture.pose,
                decomposition: &fixture.decomposition,
                common_pose: fixture.common_pose,
                common_pose_limits: CommonArticulationPoseLimitsV1::default(),
                schedule: &fixture.schedule,
                schedule_limits: CycleScheduleLimitsV1::default(),
                closure: &fixture.closure,
                paper_thickness_mm: fixture.paper_thickness_mm,
                clearance,
                clearance_limits: one_short,
                blocks: canonical_edge_partition_v1(&fixture.decomposition),
            },
        )
        .unwrap_err(),
        CommonArticulationBlockComposedPathErrorV1::Clearance(
            CommonArticulationClearanceErrorV1::ResourceLimit
        )
    );

    let fixture = prepare_strip_fixture_v1(3);
    let clearance = issue_fixture_clearance_v1(&fixture, clearance_limits);
    assert_eq!(
        issue_common_articulation_block_composed_path_authority_v1(
            CommonArticulationBlockComposedPathInputV1 {
                geometry: &fixture.geometry,
                audit: &fixture.audit,
                pose: &fixture.pose,
                decomposition: &fixture.decomposition,
                common_pose: fixture.common_pose,
                common_pose_limits: CommonArticulationPoseLimitsV1::default(),
                schedule: &fixture.schedule,
                schedule_limits: CycleScheduleLimitsV1::default(),
                closure: &fixture.closure,
                paper_thickness_mm: fixture.paper_thickness_mm,
                clearance,
                clearance_limits,
                blocks: vec![Vec::new(); BLOCK_COMPOSITION_LIMIT_V1 + 1],
            },
        )
        .unwrap_err(),
        CommonArticulationBlockComposedPathErrorV1::CanonicalBlockPartitionMismatch
    );
}

#[test]
fn staged_composition_preserves_cancel_and_deadline_v1() {
    let fixture = prepare_strip_fixture_v1(3);
    let clearance_limits = CommonArticulationClearanceLimitsV1::default();
    let clearance = issue_fixture_clearance_v1(&fixture, clearance_limits);
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        issue_common_articulation_block_composed_path_authority_with_control_v1(
            CommonArticulationBlockComposedPathInputV1 {
                geometry: &fixture.geometry,
                audit: &fixture.audit,
                pose: &fixture.pose,
                decomposition: &fixture.decomposition,
                common_pose: fixture.common_pose,
                common_pose_limits: CommonArticulationPoseLimitsV1::default(),
                schedule: &fixture.schedule,
                schedule_limits: CycleScheduleLimitsV1::default(),
                closure: &fixture.closure,
                paper_thickness_mm: fixture.paper_thickness_mm,
                clearance,
                clearance_limits,
                blocks: canonical_edge_partition_v1(&fixture.decomposition),
            },
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        )
        .unwrap_err(),
        CommonArticulationBlockComposedPathErrorV1::Cancelled
    );

    let fixture = prepare_strip_fixture_v1(3);
    let clearance = issue_fixture_clearance_v1(&fixture, clearance_limits);
    assert_eq!(
        issue_common_articulation_block_composed_path_authority_with_control_v1(
            CommonArticulationBlockComposedPathInputV1 {
                geometry: &fixture.geometry,
                audit: &fixture.audit,
                pose: &fixture.pose,
                decomposition: &fixture.decomposition,
                common_pose: fixture.common_pose,
                common_pose_limits: CommonArticulationPoseLimitsV1::default(),
                schedule: &fixture.schedule,
                schedule_limits: CycleScheduleLimitsV1::default(),
                closure: &fixture.closure,
                paper_thickness_mm: fixture.paper_thickness_mm,
                clearance,
                clearance_limits,
                blocks: canonical_edge_partition_v1(&fixture.decomposition),
            },
            &CooperativeOperationControlV1::new(None, Instant::now()),
        )
        .unwrap_err(),
        CommonArticulationBlockComposedPathErrorV1::DeadlineExceeded
    );
}

#[test]
fn final_continuous_layer_path_is_positive_and_stops_at_permission_boundary_v1() {
    let authority = prepare_final_path_fixture_v1()
        .issue(
            CommonArticulationClearanceLimitsV1::default(),
            false,
            None,
            &CooperativeOperationControlV1::unbounded(),
        )
        .expect("final common-articulation continuous-layer path");
    assert_eq!(
        authority.model_id(),
        COMMON_ARTICULATION_CONTINUOUS_LAYER_PATH_MODEL_ID_V1
    );
    assert_eq!(authority.block_count_v1(), 2);
    assert_ne!(authority.binding_fingerprint_v1(), [0; 32]);
    assert!(authority.authorizes_continuous_motion());
    assert!(authority.authorizes_collision_clearance());
    assert!(authority.authorizes_layer_transport());
    assert!(!authority.authorizes_project_mutation());
    assert!(!authority.authorizes_apply());
    assert!(!authority.authorizes_viewer());
}

#[test]
fn final_continuous_layer_path_accepts_three_four_five_and_eight_blocks_v1() {
    for block_count in [3, 4, 5, 8] {
        let fixture = prepare_final_path_fixture_with_variants_v1(false, false, block_count)
            .issue_for_revalidation();
        let block_sources = fixture.block_sources.iter().collect::<Vec<_>>();
        fixture
            .authority
            .revalidate_v1(fixture.input(&block_sources))
            .expect("exact multi-block final authority revalidation");
        assert_eq!(fixture.authority.block_count_v1(), block_count);
        assert!(fixture.authority.authorizes_continuous_motion());
        assert!(fixture.authority.authorizes_collision_clearance());
        assert!(fixture.authority.authorizes_layer_transport());
        assert!(!fixture.authority.authorizes_project_mutation());
        assert!(!fixture.authority.authorizes_apply());
        assert!(!fixture.authority.authorizes_viewer());
    }
}

#[test]
fn final_authority_revalidates_exact_live_inputs_v1() {
    let fixture = prepare_final_path_fixture_v1().issue_for_revalidation();
    let block_sources = fixture.block_sources.iter().collect::<Vec<_>>();
    fixture
        .authority
        .revalidate_v1(fixture.input(&block_sources))
        .expect("exact retained final authority revalidation");
}

#[test]
fn final_revalidation_rejects_bit_mutations_foreign_live_input_and_binding_mismatch_v1() {
    let fixture = prepare_final_path_fixture_v1().issue_for_revalidation();
    let block_sources = fixture.block_sources.iter().collect::<Vec<_>>();

    let mut target_angles = fixture.target_angles.clone();
    target_angles[0].1 = f64::from_bits(target_angles[0].1.to_bits() ^ 1);
    let mut input = fixture.input(&block_sources);
    input.target_angles = &target_angles;
    assert_eq!(
        fixture.authority.revalidate_v1(input).unwrap_err(),
        CommonArticulationContinuousLayerPathErrorV1::CompleteMultiBlockMismatch
    );

    let mut input = fixture.input(&block_sources);
    input.paper_thickness_mm = f64::from_bits(fixture.paper_thickness_mm.to_bits() ^ 1);
    assert!(matches!(
        fixture.authority.revalidate_v1(input),
        Err(CommonArticulationContinuousLayerPathErrorV1::Staged(_))
    ));

    let mut input = fixture.input(&block_sources);
    input.issuer_context[0] ^= 1;
    assert_eq!(
        fixture.authority.revalidate_v1(input).unwrap_err(),
        CommonArticulationContinuousLayerPathErrorV1::CompleteMultiBlockMismatch
    );

    let mut input = fixture.input(&block_sources);
    input.articulation_layer_fingerprint[0] ^= 1;
    assert_eq!(
        fixture.authority.revalidate_v1(input).unwrap_err(),
        CommonArticulationContinuousLayerPathErrorV1::CompleteMultiBlockMismatch
    );

    let foreign = prepare_cactus_fixture_v1(2);
    let mut input = fixture.input(&block_sources);
    input.geometry = &foreign.geometry;
    assert!(matches!(
        fixture.authority.revalidate_v1(input),
        Err(CommonArticulationContinuousLayerPathErrorV1::Staged(_))
    ));

    let mut corrupt = prepare_final_path_fixture_v1().issue_for_revalidation();
    corrupt.authority.corrupt_binding_for_test_v1();
    let corrupt_block_sources = corrupt.block_sources.iter().collect::<Vec<_>>();
    assert_eq!(
        corrupt
            .authority
            .revalidate_v1(corrupt.input(&corrupt_block_sources))
            .unwrap_err(),
        CommonArticulationContinuousLayerPathErrorV1::BindingMismatch
    );
}

#[test]
fn final_revalidation_stops_at_entry_midpoint_and_final_checkpoint_v1() {
    let fixture =
        prepare_final_path_fixture_with_variants_v1(false, false, 3).issue_for_revalidation();
    let block_sources = fixture.block_sources.iter().collect::<Vec<_>>();
    let input = fixture.input(&block_sources);

    let cancelled = AtomicBool::new(true);
    assert_eq!(
        fixture.authority.revalidate_with_control_v1(
            input,
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(CommonArticulationContinuousLayerPathErrorV1::Cancelled)
    );
    assert_eq!(
        fixture.authority.revalidate_with_control_v1(
            input,
            &CooperativeOperationControlV1::new(None, Instant::now()),
        ),
        Err(CommonArticulationContinuousLayerPathErrorV1::DeadlineExceeded)
    );

    let mut checkpoint_count = 0usize;
    fixture
        .authority
        .revalidate_with_checkpoint_for_test_v1(input, || {
            checkpoint_count += 1;
            Ok(())
        })
        .expect("count final revalidation checkpoints");
    assert!(checkpoint_count > 8);

    for stop_at in [1, checkpoint_count / 2, checkpoint_count] {
        for expected in [
            CommonArticulationContinuousLayerPathErrorV1::Cancelled,
            CommonArticulationContinuousLayerPathErrorV1::DeadlineExceeded,
        ] {
            let mut observed = 0usize;
            assert_eq!(
                fixture
                    .authority
                    .revalidate_with_checkpoint_for_test_v1(input, || {
                        observed += 1;
                        if observed == stop_at {
                            Err(expected)
                        } else {
                            Ok(())
                        }
                    }),
                Err(expected)
            );
            assert_eq!(observed, stop_at);
        }
    }
}

#[test]
fn final_path_rejects_foreign_and_replayed_whole_parent_layer_sources_v1() {
    let foreign_source = prepare_strip_fixture_v1(3).source;
    assert_eq!(
        prepare_final_path_fixture_v1()
            .issue(
                CommonArticulationClearanceLimitsV1::default(),
                false,
                Some(foreign_source.as_ref()),
                &CooperativeOperationControlV1::unbounded(),
            )
            .unwrap_err(),
        CommonArticulationContinuousLayerPathErrorV1::WholeParentLayerMismatch
    );
    assert_eq!(
        prepare_final_path_fixture_v1()
            .issue(
                CommonArticulationClearanceLimitsV1::default(),
                true,
                None,
                &CooperativeOperationControlV1::unbounded(),
            )
            .unwrap_err(),
        CommonArticulationContinuousLayerPathErrorV1::WholeParentLayerMismatch
    );
}

#[test]
fn final_path_rejects_partition_replay_and_one_short_clearance_v1() {
    let mut partition_replay = prepare_final_path_fixture_v1();
    partition_replay.block_sources.swap(0, 1);
    assert_eq!(
        partition_replay
            .issue(
                CommonArticulationClearanceLimitsV1::default(),
                false,
                None,
                &CooperativeOperationControlV1::unbounded(),
            )
            .unwrap_err(),
        CommonArticulationContinuousLayerPathErrorV1::CompleteMultiBlockMismatch
    );

    let one_short = prepare_final_path_fixture_v1();
    let clearance_limits = CommonArticulationClearanceLimitsV1 {
        max_cross_block_pairs: one_short.clearance_pair_count - 1,
        ..CommonArticulationClearanceLimitsV1::default()
    };
    assert_eq!(
        one_short
            .issue(
                clearance_limits,
                false,
                None,
                &CooperativeOperationControlV1::unbounded(),
            )
            .unwrap_err(),
        CommonArticulationContinuousLayerPathErrorV1::Staged(
            CommonArticulationBlockComposedPathErrorV1::Clearance(
                CommonArticulationClearanceErrorV1::ResourceLimit
            )
        )
    );
}

#[test]
fn final_path_rejects_same_stationary_path_with_different_exact_representation_v1() {
    assert_eq!(
        prepare_final_path_fixture_with_noncanonical_block_schedules_v1(true)
            .issue(
                CommonArticulationClearanceLimitsV1::default(),
                false,
                None,
                &CooperativeOperationControlV1::unbounded(),
            )
            .unwrap_err(),
        CommonArticulationContinuousLayerPathErrorV1::BlockScheduleRestrictionMismatch
    );
}

#[test]
fn final_path_rejects_foreign_block_source_with_the_same_face_ids_v1() {
    assert_eq!(
        prepare_final_path_fixture_with_variants_v1(false, true, 2)
            .issue(
                CommonArticulationClearanceLimitsV1::default(),
                false,
                None,
                &CooperativeOperationControlV1::unbounded(),
            )
            .unwrap_err(),
        CommonArticulationContinuousLayerPathErrorV1::BlockSourceRestrictionMismatch
    );
}

#[test]
fn final_path_rejects_a_full_schedule_that_cannot_restrict_to_every_block_v1() {
    assert_eq!(
        prepare_final_path_fixture_with_variants_v1(true, false, 3)
            .issue(
                CommonArticulationClearanceLimitsV1::default(),
                false,
                None,
                &CooperativeOperationControlV1::unbounded(),
            )
            .unwrap_err(),
        CommonArticulationContinuousLayerPathErrorV1::BlockScheduleRestrictionMismatch
    );
}

#[test]
fn final_path_preserves_cancel_and_deadline_without_authority_v1() {
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        prepare_final_path_fixture_v1()
            .issue(
                CommonArticulationClearanceLimitsV1::default(),
                false,
                None,
                &CooperativeOperationControlV1::new(
                    Some(&cancelled),
                    Instant::now() + Duration::from_secs(1),
                ),
            )
            .unwrap_err(),
        CommonArticulationContinuousLayerPathErrorV1::Cancelled
    );
    assert_eq!(
        prepare_final_path_fixture_v1()
            .issue(
                CommonArticulationClearanceLimitsV1::default(),
                false,
                None,
                &CooperativeOperationControlV1::new(None, Instant::now()),
            )
            .unwrap_err(),
        CommonArticulationContinuousLayerPathErrorV1::DeadlineExceeded
    );
}
