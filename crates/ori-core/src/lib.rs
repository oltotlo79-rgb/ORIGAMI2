use ori_domain::{CreasePattern, Edge, EdgeId, EdgeKind, Point2, Vertex, VertexId};

mod applied_pose;
mod constraints;
mod editor;
mod fold_model_fingerprint;
mod sheet;
mod stacked_fold;
mod topology;
mod validation;

pub use applied_pose::{
    APPLIED_POSE_MODEL_ID_V1, AppliedHingeAngleV1, AppliedPoseErrorV1, AppliedPoseLimitsV1,
    AppliedPoseResourceV1, AppliedPoseV1, prepare_applied_pose_v1,
};
pub use constraints::{
    ConstraintEdgeRoleV1, ConstraintId, ConstraintPreflightV1, ConstraintScalarFieldV1,
    ConstraintVertexRoleV1, DEFAULT_MAX_CONSTRAINT_EDGES, DEFAULT_MAX_CONSTRAINT_PRECHECKS,
    DEFAULT_MAX_CONSTRAINT_RECORDS, DEFAULT_MAX_CONSTRAINT_REFERENCES,
    DEFAULT_MAX_CONSTRAINT_VERTICES, DirectConstraintConflictKindV1, DirectConstraintConflictV1,
    GEOMETRIC_CONSTRAINT_MODEL_ID_V1, GEOMETRIC_CONSTRAINT_SCHEMA_VERSION_V1,
    GeometricConstraintDocumentV1, GeometricConstraintDocumentValidationErrorV1,
    GeometricConstraintErrorV1, GeometricConstraintKindV1, GeometricConstraintLimitsV1,
    GeometricConstraintRecordV1, GeometricConstraintResourceV1, GeometricConstraintSetV1,
    GeometricConstraintUnknownReasonV1, MAX_DIRECT_CONFLICT_CAUSE_IDS_V1,
    preflight_direct_conflicts_v1, prepare_geometric_constraints_v1,
    validate_geometric_constraint_document_v1,
    validate_geometric_constraint_record_against_pattern_v1,
};
pub use editor::{
    Command, CommandError, CommandResult, EDITOR_HISTORY_SCHEMA_VERSION_V1, EditorHistoryErrorV1,
    EditorHistoryV1, EditorState, ElementMetadataTargetV1, HistoryEntryLimitError,
    IntersectionEdgeTarget, JunctionVertexIntent, MAX_EDITOR_HISTORY_ENTRIES, MAX_REVISION,
    Revision, VertexPositionUpdate,
};
pub use fold_model_fingerprint::fold_model_fingerprint_v1;
pub use ori_foldability::{
    DEFAULT_MAX_ARRANGEMENT_SEGMENTS, DEFAULT_MAX_CERTIFICATE_BYTES, DEFAULT_MAX_CONSTRAINTS,
    DEFAULT_MAX_EDGE_INCIDENCE_RECORDS, DEFAULT_MAX_EXACT_INTEGER_BITS,
    DEFAULT_MAX_EXACT_OPERATIONS, DEFAULT_MAX_FACE_BOUNDARY_HALF_EDGES, DEFAULT_MAX_FACES,
    DEFAULT_MAX_HINGES, DEFAULT_MAX_LOCAL_VERTICES, DEFAULT_MAX_OVERLAP_CELLS,
    DEFAULT_MAX_OVERLAP_FACE_PAIRS, DEFAULT_MAX_PAPER_BOUNDARY_VERTICES, DEFAULT_MAX_SEARCH_NODES,
    DEFAULT_MAX_SOURCE_EDGES, DEFAULT_MAX_SOURCE_VERTICES, DEFAULT_MAX_TOTAL_RECORDS,
    ExactAffineTransform, ExactPointValue, ExactRationalValue, ExactSign, FacePairOrderSnapshot,
    FacewiseConstraintKind, FacewiseProofSummary, FlatFoldabilityInputArtifact,
    FlatFoldabilityInputConsistencyIssue, FlatFoldabilityProofIncompleteReason,
    FlatFoldabilityResource, FoldModelFingerprintV1, FoldedFaceOrientation, FoldedFaceSnapshot,
    GLOBAL_FLAT_FOLDABILITY_MODEL_ID, GlobalFlatFoldabilityCheckpoint,
    GlobalFlatFoldabilityExecutionControl, GlobalFlatFoldabilityExecutionError,
    GlobalFlatFoldabilityImpossibleReason, GlobalFlatFoldabilityInput,
    GlobalFlatFoldabilityInternalError, GlobalFlatFoldabilityLimits, GlobalFlatFoldabilityModelId,
    GlobalFlatFoldabilityObserver, GlobalFlatFoldabilityOutcome, GlobalFlatFoldabilityPhase,
    GlobalFlatFoldabilityPossibleReason, GlobalFlatFoldabilityProgress,
    GlobalFlatFoldabilityProvenance, GlobalFlatFoldabilityReport,
    GlobalFlatFoldabilityUnknownReason, GlobalFlatFoldabilityVerdict,
    GlobalFlatFoldabilityWorkCounts, LAYER_ORDER_MODEL_ID, LayerFace, LayerOrderDerivation,
    LayerOrderModelId, LayerOrderProvenance, LayerOrderSnapshot, LocalNecessaryConditionViolation,
    NoopGlobalFlatFoldabilityObserver, OverlapCellKey, OverlapCellSnapshot,
    UnsupportedFlatFoldabilityTopology, analyze_global_flat_foldability,
    analyze_global_flat_foldability_with_control, analyze_global_flat_foldability_with_observer,
};
pub use ori_geometry::{
    BoundaryEdgeRef, CreasePatternValidation, EdgeEndpoint, GeometryError, PaperValidationIssue,
    PointPolygonRelation, SegmentIntersection, ValidationIssue, segment_midpoint_polygon_relation,
    validate_paper,
};
pub use ori_topology::{
    CooperativeAnalysisAbort, CooperativeAnalysisCheckpoint, FaceExtractionReport,
    LocalFlatFoldabilityModel, LocalFlatFoldabilityReport, LocalFlatFoldabilityReportStatus,
    LocalFoldabilityConditionStatus, LocalFoldabilityReason, LocalVertexFoldability,
    LocalVertexFoldabilityVerdict, MAX_EXACT_FOLD_DEGREE, TopologyIssue, TopologyIssueKind,
    TopologyIssueSeverity, TopologySnapshot, analyze_local_flat_foldability,
    analyze_local_flat_foldability_with_checkpoint,
};
pub use sheet::{SheetCreationError, SheetProject, create_rectangular_sheet};
pub use stacked_fold::{
    DEFAULT_MAX_FACE_LINEAGE_BOUNDARY_HALF_EDGES, DEFAULT_MAX_FACE_LINEAGE_EXACT_CONTAINMENT_TESTS,
    DEFAULT_MAX_FACE_LINEAGE_FACE_PAIRS, DEFAULT_MAX_FACE_LINEAGE_SOURCE_FACES,
    DEFAULT_MAX_FACE_LINEAGE_TARGET_FACES, DEFAULT_MAX_STACKED_FOLD_BUILD_CARRIERS,
    DEFAULT_MAX_STACKED_FOLD_BUILD_EDGES, DEFAULT_MAX_STACKED_FOLD_BUILD_INTERSECTIONS,
    DEFAULT_MAX_STACKED_FOLD_BUILD_VERTICES, DEFAULT_MAX_STACKED_FOLD_CARRIER_OVERLAP_TESTS,
    DEFAULT_MAX_STACKED_FOLD_EDGE_CARRIER_TESTS, DEFAULT_MAX_STACKED_FOLD_EXPECTED_CREASES,
    DEFAULT_MAX_STACKED_FOLD_LINEAGE_DESCENDANTS, DEFAULT_MAX_STACKED_FOLD_LINEAGE_RECORDS,
    ExpectedCreaseSubdivisionV1, ExpectedStackedFoldCreaseV1, FaceLineageError, FaceLineageInput,
    FaceLineageLimits, FaceLineageRecord, FaceLineageResource, FaceLineageTopology, FaceLineageV1,
    PrepareStackedFoldGeometryErrorV1, PrepareStackedFoldInitialPoseErrorV1,
    PrepareStackedFoldRequestedPoseErrorV1, PrepareStackedFoldTargetGraphAuditErrorV1,
    PrepareStackedFoldTargetModelErrorV1, PreparedStackedFoldGeometryV1,
    PreparedStackedFoldInitialGraphPoseV1, PreparedStackedFoldInitialPoseV1,
    PreparedStackedFoldRequestedGraphPoseV1, PreparedStackedFoldRequestedPoseV1,
    PreparedStackedFoldTargetGraphAuditV1, PreparedStackedFoldTargetModelV1,
    STACKED_FOLD_GRAPH_CLOSURE_TOLERANCE_V1, STACKED_FOLD_TARGET_GRAPH_AUDIT_MODEL_ID_V1,
    SourceEdgeSubdivisionV1, StackedFoldGeometryCarrierV1, StackedFoldGeometryErrorV1,
    StackedFoldGeometryInputV1, StackedFoldGeometryLimitsV1, StackedFoldGeometryProofV1,
    StackedFoldGeometryResourceV1, StackedFoldTopologyBuildErrorV1,
    StackedFoldTopologyBuildLimitsV1, StackedFoldTopologyBuildResourceV1,
    StackedFoldTopologyCandidateV1, build_stacked_fold_topology_v1, prepare_face_lineage_v1,
    prepare_stacked_fold_geometry_candidate_v1, prepare_stacked_fold_geometry_v1,
    prepare_stacked_fold_initial_graph_pose_v1, prepare_stacked_fold_initial_pose_v1,
    prepare_stacked_fold_requested_graph_pose_v1, prepare_stacked_fold_requested_pose_v1,
    prepare_stacked_fold_target_graph_audit_v1, prepare_stacked_fold_target_model_v1,
};
pub use topology::{EditorTopology, TopologyAnalysisInput};
pub use validation::EditorValidation;

#[must_use]
pub fn benchmark_pattern(edge_count: usize) -> CreasePattern {
    if edge_count == 0 {
        return CreasePattern::empty();
    }
    let mut side = ((edge_count as f64 / 2.0).sqrt().ceil() as usize).max(2);
    while 2 * side * (side - 1) < edge_count {
        side += 1;
    }
    let mut vertices = Vec::with_capacity(side * side);
    for y in 0..side {
        for x in 0..side {
            vertices.push(Vertex {
                id: VertexId::new(),
                position: Point2::new(x as f64, y as f64),
            });
        }
    }
    let mut edges = Vec::with_capacity(edge_count);
    'grid: for y in 0..side {
        for x in 0..side {
            let index = y * side + x;
            if x + 1 < side {
                edges.push(Edge {
                    id: EdgeId::new(),
                    start: vertices[index].id,
                    end: vertices[index + 1].id,
                    kind: if y % 2 == 0 {
                        EdgeKind::Mountain
                    } else {
                        EdgeKind::Valley
                    },
                });
                if edges.len() == edge_count {
                    break 'grid;
                }
            }
            if y + 1 < side {
                edges.push(Edge {
                    id: EdgeId::new(),
                    start: vertices[index].id,
                    end: vertices[index + side].id,
                    kind: if x % 2 == 0 {
                        EdgeKind::Valley
                    } else {
                        EdgeKind::Mountain
                    },
                });
                if edges.len() == edge_count {
                    break 'grid;
                }
            }
        }
    }
    CreasePattern { vertices, edges }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_ten_thousand_edge_fixture() {
        let pattern = benchmark_pattern(10_000);
        assert_eq!(pattern.edges.len(), 10_000);
    }
}
