use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    io::{self, Write},
};

use num_rational::BigRational;
use num_traits::{Signed, Zero};
use ori_domain::{CreasePattern, EdgeId, EdgeKind, Paper, VertexId};
use ori_topology::{FoldAssignment, TopologySnapshot};
use sha2::{Digest, Sha256};

use crate::{
    FacePairOrderSnapshot, FacewiseConstraintKind, FacewiseProofSummary,
    FlatFoldabilityProofIncompleteReason, FlatFoldabilityResource, FoldedFaceOrientation,
    FoldedFaceSnapshot, GlobalFlatFoldabilityCheckpoint, GlobalFlatFoldabilityExecutionError,
    GlobalFlatFoldabilityImpossibleReason, GlobalFlatFoldabilityInternalError,
    GlobalFlatFoldabilityLimits, GlobalFlatFoldabilityObserver, GlobalFlatFoldabilityOutcome,
    GlobalFlatFoldabilityPhase, GlobalFlatFoldabilityPossibleReason,
    GlobalFlatFoldabilityProvenance, GlobalFlatFoldabilityReport,
    GlobalFlatFoldabilityUnknownReason, GlobalFlatFoldabilityWorkCounts, LayerFace,
    LayerOrderDerivation, LayerOrderModelId, LayerOrderProvenance, LayerOrderSnapshot,
    OverlapCellKey, OverlapCellSnapshot, UnsupportedFlatFoldabilityTopology, complete_progress,
    constraints::{
        ConstraintSolverControl, ConstraintSolverEvent, ConstraintSolverResult, TupleConstraint,
        solve_constraints_with_memory,
    },
    exact::{
        self, ExactBudget, ExactError, Point, Rational, Transform, add, apply, average3, cmp,
        compose, cross, div, midpoint, mul, point_from_binary64, rational_bytes, reflection_across,
        signed_double_area, sub,
    },
    unknown,
};

const CELL_KEY_DOMAIN: &[u8] = b"ORIGAMI2\0overlap-cell\0v1\0";
const CONTROL_POLL_RECORDS: usize = 1_024;
const SERIALIZATION_POLL_BYTES: usize = 64 * 1_024;
const TACO_TACO_VALID_SOURCE_TUPLES: [&str; 16] = [
    "111112", "111121", "111222", "112111", "121112", "121222", "122111", "122212", "211121",
    "211222", "212111", "212221", "221222", "222111", "222212", "222221",
];

type FacewiseResult<T> = Result<T, FacewiseAbort>;

#[derive(Debug, Default, Clone, Copy)]
struct ExactStorage {
    embedding_bytes: usize,
    arrangement_bytes: usize,
    snapshot_bytes: usize,
    certificate_structure_bytes: usize,
    verification_bytes: usize,
    constraint_bytes: usize,
}

impl ExactStorage {
    fn total(&self) -> Option<usize> {
        self.embedding_bytes
            .checked_add(self.arrangement_bytes)?
            .checked_add(self.snapshot_bytes)?
            .checked_add(self.certificate_structure_bytes)?
            .checked_add(self.verification_bytes)?
            .checked_add(self.constraint_bytes)
    }
}

#[derive(Debug)]
enum FacewiseAbort {
    Unknown(GlobalFlatFoldabilityUnknownReason),
    Impossible(GlobalFlatFoldabilityImpossibleReason),
    Execution(GlobalFlatFoldabilityExecutionError),
}

impl From<GlobalFlatFoldabilityExecutionError> for FacewiseAbort {
    fn from(value: GlobalFlatFoldabilityExecutionError) -> Self {
        Self::Execution(value)
    }
}

impl From<ExactError> for FacewiseAbort {
    fn from(value: ExactError) -> Self {
        match value {
            ExactError::NonFiniteBinary64 | ExactError::NegativeZero => {
                Self::Unknown(GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::InvalidBinary64Coordinate,
                })
            }
            ExactError::DegenerateDivision => {
                Self::Unknown(GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::NonSimpleFace,
                })
            }
            ExactError::IntegerBitLimitReached {
                limit_bits,
                observed_bits,
            } => Self::Unknown(
                GlobalFlatFoldabilityUnknownReason::ExactNumberLimitReached {
                    limit_bits,
                    observed_bits,
                },
            ),
            ExactError::WorkLimitReached { limit, observed } => {
                Self::Unknown(GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::ExactOperations,
                    limit,
                    observed,
                })
            }
            ExactError::DeadlineReached { phase } => {
                Self::Unknown(GlobalFlatFoldabilityUnknownReason::TimeLimitReached { phase })
            }
            ExactError::Cancelled => {
                Self::Execution(GlobalFlatFoldabilityExecutionError::Cancelled)
            }
            ExactError::InternalFailure => {
                Self::Execution(GlobalFlatFoldabilityExecutionError::Internal {
                    reason: GlobalFlatFoldabilityInternalError::ValidatedTopologyInvariantLost,
                })
            }
        }
    }
}

struct Runtime<'a, O: GlobalFlatFoldabilityObserver + ?Sized> {
    observer: &'a mut O,
    limits: GlobalFlatFoldabilityLimits,
    work: GlobalFlatFoldabilityWorkCounts,
    phase: GlobalFlatFoldabilityPhase,
    exact_storage: ExactStorage,
}

impl<'a, O: GlobalFlatFoldabilityObserver + ?Sized> Runtime<'a, O> {
    fn new(
        observer: &'a mut O,
        limits: GlobalFlatFoldabilityLimits,
        work: GlobalFlatFoldabilityWorkCounts,
    ) -> Self {
        Self {
            observer,
            limits,
            work,
            phase: GlobalFlatFoldabilityPhase::ValidatingLocalConditions,
            exact_storage: ExactStorage::default(),
        }
    }

    fn advance(
        &mut self,
        phase: GlobalFlatFoldabilityPhase,
        total_work: Option<usize>,
    ) -> FacewiseResult<()> {
        if phase <= self.phase {
            return Err(FacewiseAbort::Execution(
                GlobalFlatFoldabilityExecutionError::Internal {
                    reason: GlobalFlatFoldabilityInternalError::ValidatedTopologyInvariantLost,
                },
            ));
        }
        self.phase = phase;
        self.progress();
        self.checkpoint(total_work)
    }

    fn checkpoint(&mut self, _total_work: Option<usize>) -> FacewiseResult<()> {
        match self.observer.checkpoint() {
            GlobalFlatFoldabilityCheckpoint::Continue => Ok(()),
            GlobalFlatFoldabilityCheckpoint::DeadlineReached => Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::TimeLimitReached { phase: self.phase },
            )),
            GlobalFlatFoldabilityCheckpoint::Cancelled => Err(FacewiseAbort::Execution(
                GlobalFlatFoldabilityExecutionError::Cancelled,
            )),
        }
    }

    fn poll_control(&mut self, pending_records: &mut usize) -> FacewiseResult<()> {
        *pending_records = pending_records.saturating_add(1);
        if *pending_records >= CONTROL_POLL_RECORDS {
            *pending_records = 0;
            self.checkpoint(None)?;
        }
        Ok(())
    }

    fn progress(&mut self) {
        self.observer
            .on_progress(crate::GlobalFlatFoldabilityProgress {
                phase: self.phase,
                completed_work: self
                    .work
                    .total_records
                    .saturating_add(self.work.arrangement_segments)
                    .saturating_add(self.work.constraints)
                    .saturating_add(self.work.search_nodes),
                total_work: None,
                exact_operations: self.work.exact_operations,
                overlap_face_pairs: self.work.overlap_face_pairs,
                overlap_cells: self.work.overlap_cells,
                constraints: self.work.constraints,
                search_nodes: self.work.search_nodes,
            });
    }

    fn set_overlap_pairs(&mut self, observed: usize) -> FacewiseResult<()> {
        if observed > self.limits.max_overlap_face_pairs {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::OverlapFacePairs,
                    limit: self.limits.max_overlap_face_pairs,
                    observed,
                },
            ));
        }
        self.work.overlap_face_pairs = observed;
        Ok(())
    }

    fn set_arrangement_segments(&mut self, observed: usize) -> FacewiseResult<()> {
        if observed > self.limits.max_arrangement_segments {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::OverlapArrangementLimitReached {
                    resource: FlatFoldabilityResource::ArrangementSegments,
                    limit: self.limits.max_arrangement_segments,
                    observed,
                },
            ));
        }
        self.work.arrangement_segments = observed;
        Ok(())
    }

    fn set_overlap_cells(&mut self, observed: usize) -> FacewiseResult<()> {
        if observed > self.limits.max_overlap_cells {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::OverlapArrangementLimitReached {
                    resource: FlatFoldabilityResource::OverlapCells,
                    limit: self.limits.max_overlap_cells,
                    observed,
                },
            ));
        }
        self.work.overlap_cells = observed;
        Ok(())
    }

    fn set_constraints(&mut self, observed: usize) -> FacewiseResult<()> {
        if observed > self.limits.max_constraints {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ConstraintLimitReached {
                    limit: self.limits.max_constraints,
                    observed,
                },
            ));
        }
        self.work.constraints = observed;
        Ok(())
    }

    fn set_search_nodes(&mut self, observed: usize) -> FacewiseResult<()> {
        if observed > self.limits.max_search_nodes {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::SearchNodes,
                    limit: self.limits.max_search_nodes,
                    observed,
                },
            ));
        }
        self.work.search_nodes = observed;
        Ok(())
    }

    fn set_certificate_bytes(&mut self, observed: usize) -> FacewiseResult<()> {
        if observed > self.limits.max_certificate_bytes {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit: self.limits.max_certificate_bytes,
                    observed,
                },
            ));
        }
        self.work.certificate_bytes = observed;
        Ok(())
    }

    fn exact_storage_limit_failure(&self, observed: usize) -> FacewiseAbort {
        FacewiseAbort::Unknown(GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
            resource: FlatFoldabilityResource::CertificateBytes,
            limit: self.limits.max_certificate_bytes,
            observed,
        })
    }

    fn ensure_storage_values(
        &self,
        storage: ExactStorage,
        transient_bytes: usize,
    ) -> FacewiseResult<usize> {
        let values = [
            storage.embedding_bytes,
            storage.arrangement_bytes,
            storage.snapshot_bytes,
            storage.certificate_structure_bytes,
            storage.verification_bytes,
            storage.constraint_bytes,
            transient_bytes,
        ];
        // `usize::MAX` is also the sentinel produced by the storage-size
        // helpers on arithmetic overflow. Reject it even when a caller
        // configures an unbounded-looking `usize::MAX` limit.
        if values.contains(&usize::MAX) {
            return Err(self.exact_storage_limit_failure(usize::MAX));
        }
        let observed = values.into_iter().try_fold(0_usize, |total, value| {
            total
                .checked_add(value)
                .ok_or_else(|| self.exact_storage_limit_failure(usize::MAX))
        })?;
        if observed > self.limits.max_certificate_bytes {
            return Err(self.exact_storage_limit_failure(observed));
        }
        Ok(observed)
    }

    fn set_embedding_exact_storage(&mut self, observed: usize) -> FacewiseResult<()> {
        let mut candidate = self.exact_storage;
        candidate.embedding_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.embedding_bytes = observed;
        Ok(())
    }

    fn add_embedding_exact_storage(&mut self, additional: usize) -> FacewiseResult<()> {
        let observed = self
            .exact_storage
            .embedding_bytes
            .saturating_add(additional);
        let mut candidate = self.exact_storage;
        candidate.embedding_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.embedding_bytes = observed;
        Ok(())
    }

    fn set_arrangement_exact_storage(&mut self, observed: usize) -> FacewiseResult<()> {
        let mut candidate = self.exact_storage;
        candidate.arrangement_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.arrangement_bytes = observed;
        Ok(())
    }

    fn add_snapshot_exact_storage(&mut self, additional: usize) -> FacewiseResult<()> {
        let observed = self.exact_storage.snapshot_bytes.saturating_add(additional);
        let mut candidate = self.exact_storage;
        candidate.snapshot_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.snapshot_bytes = observed;
        Ok(())
    }

    fn ensure_transient_exact_storage(&self, additional: usize) -> FacewiseResult<()> {
        self.ensure_storage_values(self.exact_storage, additional)?;
        Ok(())
    }

    fn add_certificate_structure_storage(&mut self, additional: usize) -> FacewiseResult<()> {
        let observed = self
            .exact_storage
            .certificate_structure_bytes
            .checked_add(additional)
            .ok_or_else(|| self.exact_storage_limit_failure(usize::MAX))?;
        let mut candidate = self.exact_storage;
        candidate.certificate_structure_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.certificate_structure_bytes = observed;
        Ok(())
    }

    fn add_verification_storage(&mut self, additional: usize) -> FacewiseResult<()> {
        let observed = self
            .exact_storage
            .verification_bytes
            .checked_add(additional)
            .ok_or_else(|| self.exact_storage_limit_failure(usize::MAX))?;
        let mut candidate = self.exact_storage;
        candidate.verification_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.verification_bytes = observed;
        Ok(())
    }

    fn add_constraint_storage(&mut self, additional: usize) -> FacewiseResult<()> {
        let observed = self
            .exact_storage
            .constraint_bytes
            .checked_add(additional)
            .ok_or_else(|| self.exact_storage_limit_failure(usize::MAX))?;
        let mut candidate = self.exact_storage;
        candidate.constraint_bytes = observed;
        self.ensure_storage_values(candidate, 0)?;
        self.exact_storage.constraint_bytes = observed;
        Ok(())
    }

    fn ensure_constraint_transient_storage(&self, additional: usize) -> FacewiseResult<()> {
        self.ensure_storage_values(self.exact_storage, additional)?;
        Ok(())
    }

    fn clear_constraint_storage(&mut self) {
        self.exact_storage.constraint_bytes = 0;
    }

    fn remaining_storage_bytes(&self) -> FacewiseResult<usize> {
        let used = self
            .exact_storage
            .total()
            .ok_or_else(|| self.exact_storage_limit_failure(usize::MAX))?;
        self.limits
            .max_certificate_bytes
            .checked_sub(used)
            .ok_or_else(|| self.exact_storage_limit_failure(used))
    }

    fn clear_verification_storage(&mut self) {
        self.exact_storage.verification_bytes = 0;
    }

    fn verification_storage_bytes(&self) -> usize {
        self.exact_storage.verification_bytes
    }

    fn restore_verification_storage(&mut self, retained: usize) {
        debug_assert!(retained <= self.exact_storage.verification_bytes);
        self.exact_storage.verification_bytes = retained;
    }

    fn allocation_bytes(&self, count: usize, element_size: usize) -> FacewiseResult<usize> {
        count
            .checked_mul(element_size)
            .ok_or_else(|| self.exact_storage_limit_failure(usize::MAX))
    }

    fn constraint_solver_control(
        &mut self,
        event: ConstraintSolverEvent,
        search_nodes: usize,
    ) -> ConstraintSolverControl {
        if search_nodes <= self.limits.max_search_nodes {
            self.work.search_nodes = search_nodes;
        }
        let target_phase = match event {
            ConstraintSolverEvent::PropagationBatch => None,
            ConstraintSolverEvent::SearchNode => Some(GlobalFlatFoldabilityPhase::Searching),
            ConstraintSolverEvent::VerifyingConstraint => {
                Some(GlobalFlatFoldabilityPhase::VerifyingCertificate)
            }
        };
        if let Some(target_phase) = target_phase
            && target_phase > self.phase
        {
            self.phase = target_phase;
            self.progress();
        }
        if matches!(event, ConstraintSolverEvent::SearchNode) && search_nodes.is_multiple_of(1_024)
        {
            self.progress();
        }
        match self.observer.checkpoint() {
            GlobalFlatFoldabilityCheckpoint::Continue => ConstraintSolverControl::Continue,
            GlobalFlatFoldabilityCheckpoint::DeadlineReached => {
                ConstraintSolverControl::DeadlineReached
            }
            GlobalFlatFoldabilityCheckpoint::Cancelled => ConstraintSolverControl::Cancelled,
        }
    }
}

impl<O: GlobalFlatFoldabilityObserver + ?Sized> ExactBudget for Runtime<'_, O> {
    fn record_exact_operation(&mut self) -> Result<(), ExactError> {
        let observed =
            self.work
                .exact_operations
                .checked_add(1)
                .ok_or(ExactError::WorkLimitReached {
                    limit: self.limits.max_exact_operations,
                    observed: usize::MAX,
                })?;
        if observed > self.limits.max_exact_operations {
            return Err(ExactError::WorkLimitReached {
                limit: self.limits.max_exact_operations,
                observed,
            });
        }
        self.work.exact_operations = observed;
        if observed % 1_024 == 0 {
            self.progress();
            match self.observer.checkpoint() {
                GlobalFlatFoldabilityCheckpoint::Continue => {}
                GlobalFlatFoldabilityCheckpoint::DeadlineReached => {
                    return Err(ExactError::DeadlineReached { phase: self.phase });
                }
                GlobalFlatFoldabilityCheckpoint::Cancelled => {
                    return Err(ExactError::Cancelled);
                }
            }
        }
        Ok(())
    }

    fn record_exact_value(&mut self, value: &BigRational) -> Result<(), ExactError> {
        self.work.exact_values =
            self.work
                .exact_values
                .checked_add(1)
                .ok_or(ExactError::WorkLimitReached {
                    limit: self.limits.max_exact_operations,
                    observed: usize::MAX,
                })?;
        let observed_bits = exact::bit_len(value)?;
        if observed_bits > self.limits.max_exact_integer_bits {
            return Err(ExactError::IntegerBitLimitReached {
                limit_bits: self.limits.max_exact_integer_bits,
                observed_bits,
            });
        }
        Ok(())
    }
}

fn exact_storage_bytes_point(point: &Point) -> Result<usize, ExactError> {
    Ok(exact::rational_storage_bytes(&point.x)?
        .saturating_add(exact::rational_storage_bytes(&point.y)?))
}

fn exact_storage_bytes_points(points: &[Point]) -> Result<usize, ExactError> {
    points.iter().try_fold(0_usize, |total, point| {
        Ok(total.saturating_add(exact_storage_bytes_point(point)?))
    })
}

fn exact_storage_bytes_transform(transform: &Transform) -> Result<usize, ExactError> {
    [
        &transform.m00,
        &transform.m01,
        &transform.m10,
        &transform.m11,
        &transform.tx,
        &transform.ty,
    ]
    .into_iter()
    .try_fold(0_usize, |total, value| {
        Ok(total.saturating_add(exact::rational_storage_bytes(value)?))
    })
}

fn exact_storage_bytes_embedding(embedding: &FlatEmbedding) -> Result<usize, ExactError> {
    let mut total = 0_usize;
    for face in &embedding.faces {
        total = total
            .saturating_add(exact_storage_bytes_points(&face.source.source_polygon)?)
            .saturating_add(exact_storage_bytes_transform(&face.transform)?)
            .saturating_add(exact_storage_bytes_points(&face.polygon)?);
    }
    for hinge in &embedding.hinges {
        total = total
            .saturating_add(exact_storage_bytes_point(&hinge.first_point)?)
            .saturating_add(exact_storage_bytes_point(&hinge.second_point)?);
    }
    Ok(total)
}

#[derive(Clone)]
struct SourceEdge {
    start: VertexId,
    end: VertexId,
    kind: EdgeKind,
}

#[derive(Clone)]
struct SourceFace {
    layer: LayerFace,
    vertex_ids: Vec<VertexId>,
    source_polygon: Vec<Point>,
}

#[derive(Clone)]
struct FoldedFace {
    source: SourceFace,
    transform: Transform,
    front_up: bool,
    polygon: Vec<Point>,
}

#[derive(Clone)]
struct FoldedHinge {
    edge: EdgeId,
    first_face: usize,
    second_face: usize,
    assignment: FoldAssignment,
    first_point: Point,
    second_point: Point,
}

#[derive(Clone)]
struct FlatEmbedding {
    reference_face: usize,
    faces: Vec<FoldedFace>,
    hinges: Vec<FoldedHinge>,
    material_internal_edge_count: usize,
}

struct SolveSuccess {
    reason: GlobalFlatFoldabilityPossibleReason,
    layer_order: LayerOrderSnapshot,
}

pub(crate) struct FacewiseAnalysisInput<'a> {
    pub(crate) paper: &'a Paper,
    pub(crate) crease_pattern: &'a CreasePattern,
    pub(crate) topology: &'a TopologySnapshot,
    pub(crate) canonical_faces: &'a [LayerFace],
    pub(crate) provenance: GlobalFlatFoldabilityProvenance,
    pub(crate) work_counts: GlobalFlatFoldabilityWorkCounts,
    pub(crate) limits: GlobalFlatFoldabilityLimits,
}

pub(crate) fn analyze_facewise<O: GlobalFlatFoldabilityObserver + ?Sized>(
    input: FacewiseAnalysisInput<'_>,
    observer: &mut O,
) -> Result<GlobalFlatFoldabilityReport, GlobalFlatFoldabilityExecutionError> {
    let FacewiseAnalysisInput {
        paper,
        crease_pattern,
        topology,
        canonical_faces,
        provenance,
        work_counts,
        limits,
    } = input;
    let mut runtime = Runtime::new(observer, limits, work_counts);
    let result = solve_facewise(
        paper,
        crease_pattern,
        topology,
        canonical_faces,
        provenance,
        &mut runtime,
    );
    match result {
        Ok(success) => {
            complete_progress(runtime.observer, runtime.work);
            Ok(GlobalFlatFoldabilityReport {
                provenance,
                work_counts: runtime.work,
                outcome: GlobalFlatFoldabilityOutcome::Possible {
                    reason: success.reason,
                    layer_order: Box::new(success.layer_order),
                },
            })
        }
        Err(FacewiseAbort::Unknown(reason)) => {
            complete_progress(runtime.observer, runtime.work);
            Ok(unknown(provenance, runtime.work, reason))
        }
        Err(FacewiseAbort::Impossible(reason)) => {
            complete_progress(runtime.observer, runtime.work);
            Ok(GlobalFlatFoldabilityReport {
                provenance,
                work_counts: runtime.work,
                outcome: GlobalFlatFoldabilityOutcome::Impossible { reason },
            })
        }
        Err(FacewiseAbort::Execution(error)) => Err(error),
    }
}

fn solve_facewise<O: GlobalFlatFoldabilityObserver + ?Sized>(
    paper: &Paper,
    crease_pattern: &CreasePattern,
    topology: &TopologySnapshot,
    canonical_faces: &[LayerFace],
    provenance: GlobalFlatFoldabilityProvenance,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<SolveSuccess> {
    runtime.advance(
        GlobalFlatFoldabilityPhase::BuildingFlatEmbedding,
        Some(canonical_faces.len()),
    )?;
    let embedding =
        build_flat_embedding(paper, crease_pattern, topology, canonical_faces, runtime)?;
    runtime.advance(GlobalFlatFoldabilityPhase::BuildingOverlapArrangement, None)?;
    let overlap_pairs = build_overlap_pairs(&embedding.faces, runtime)?;
    runtime.set_overlap_pairs(overlap_pairs.len())?;
    let cells = build_overlap_cells(&embedding.faces, &overlap_pairs, runtime)?;
    runtime.set_overlap_cells(cells.len())?;
    solve_layer_order(embedding, overlap_pairs, cells, provenance, runtime)
}

fn build_flat_embedding<O: GlobalFlatFoldabilityObserver + ?Sized>(
    paper: &Paper,
    crease_pattern: &CreasePattern,
    topology: &TopologySnapshot,
    canonical_faces: &[LayerFace],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<FlatEmbedding> {
    let mut vertex_records = crease_pattern.vertices.iter().collect::<Vec<_>>();
    vertex_records.sort_unstable_by_key(|vertex| vertex.id.canonical_bytes());
    let mut vertices = HashMap::with_capacity(vertex_records.len());
    for vertex in vertex_records {
        runtime.checkpoint(None)?;
        if vertices.contains_key(&vertex.id) {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::DuplicateSourceVertex,
                },
            ));
        }
        let point = point_from_binary64(vertex.position.x, vertex.position.y, runtime)?;
        runtime.add_embedding_exact_storage(exact_storage_bytes_point(&point)?)?;
        vertices.insert(vertex.id, point);
    }
    for boundary_vertex in &paper.boundary_vertices {
        if !vertices.contains_key(boundary_vertex) {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::MissingSourceVertex,
                },
            ));
        }
    }

    let mut edge_records = crease_pattern.edges.iter().collect::<Vec<_>>();
    edge_records.sort_unstable_by_key(|edge| edge.id.canonical_bytes());
    let mut edges = HashMap::with_capacity(edge_records.len());
    for edge in edge_records {
        runtime.checkpoint(None)?;
        if edge.kind == EdgeKind::Cut {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::CutEdge,
                },
            ));
        }
        if !vertices.contains_key(&edge.start) || !vertices.contains_key(&edge.end) {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::MissingSourceVertex,
                },
            ));
        }
        if edges
            .insert(
                edge.id,
                SourceEdge {
                    start: edge.start,
                    end: edge.end,
                    kind: edge.kind,
                },
            )
            .is_some()
        {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::DuplicateSourceEdge,
                },
            ));
        }
    }

    let topology_faces = topology
        .faces
        .iter()
        .map(|face| (face.id, face))
        .collect::<HashMap<_, _>>();
    let mut source_faces = Vec::with_capacity(canonical_faces.len());
    let mut face_edge_counts = HashMap::<EdgeId, usize>::new();
    for layer in canonical_faces {
        runtime.checkpoint(None)?;
        let Some(face) = topology_faces.get(&layer.face_id).copied() else {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::DisconnectedMaterial,
                },
            ));
        };
        if face.outer.half_edges.len() < 3 {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::NonSimpleFace,
                },
            ));
        }
        let mut vertex_ids = Vec::with_capacity(face.outer.half_edges.len());
        let mut polygon = Vec::with_capacity(face.outer.half_edges.len());
        let mut unique_vertices = HashSet::with_capacity(face.outer.half_edges.len());
        for (index, half_edge) in face.outer.half_edges.iter().enumerate() {
            let next = &face.outer.half_edges[(index + 1) % face.outer.half_edges.len()];
            if half_edge.destination != next.origin || !unique_vertices.insert(half_edge.origin) {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                        reason: UnsupportedFlatFoldabilityTopology::NonSimpleFace,
                    },
                ));
            }
            let Some(source_edge) = edges.get(&half_edge.edge) else {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                        reason: UnsupportedFlatFoldabilityTopology::MissingSourceEdge,
                    },
                ));
            };
            let observed = face_edge_counts
                .get(&half_edge.edge)
                .copied()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(internal_abort)?;
            face_edge_counts.insert(half_edge.edge, observed);
            let matches_source = (source_edge.start == half_edge.origin
                && source_edge.end == half_edge.destination)
                || (source_edge.end == half_edge.origin
                    && source_edge.start == half_edge.destination);
            if !matches_source {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                        reason: UnsupportedFlatFoldabilityTopology::InconsistentSourceBoundary,
                    },
                ));
            }
            let Some(point) = vertices.get(&half_edge.origin) else {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                        reason: UnsupportedFlatFoldabilityTopology::MissingSourceVertex,
                    },
                ));
            };
            runtime.add_embedding_exact_storage(exact_storage_bytes_point(point)?)?;
            vertex_ids.push(half_edge.origin);
            polygon.push(point.clone());
        }
        validate_convex_face(*layer, &polygon, runtime)?;
        source_faces.push(SourceFace {
            layer: *layer,
            vertex_ids,
            source_polygon: polygon,
        });
    }
    let hinge_edges = topology
        .hinge_adjacency
        .iter()
        .map(|hinge| hinge.edge)
        .collect::<HashSet<_>>();
    let mut material_internal_edge_count = 0_usize;
    for (edge_id, incidence_count) in &face_edge_counts {
        let source_edge = edges.get(edge_id).ok_or_else(internal_abort)?;
        match *incidence_count {
            1 if source_edge.kind == EdgeKind::Boundary => {}
            2 if matches!(source_edge.kind, EdgeKind::Mountain | EdgeKind::Valley)
                && hinge_edges.contains(edge_id) =>
            {
                material_internal_edge_count = material_internal_edge_count
                    .checked_add(1)
                    .ok_or_else(internal_abort)?;
            }
            _ => {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                        reason: UnsupportedFlatFoldabilityTopology::UnassignedHinge,
                    },
                ));
            }
        }
    }
    if material_internal_edge_count != topology.hinge_adjacency.len() {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                reason: UnsupportedFlatFoldabilityTopology::UnassignedHinge,
            },
        ));
    }

    let face_indexes = source_faces
        .iter()
        .enumerate()
        .map(|(index, face)| (face.layer.face_id, index))
        .collect::<HashMap<_, _>>();
    let mut adjacency = vec![Vec::new(); source_faces.len()];
    for hinge in &topology.hinge_adjacency {
        let Some(&first) = face_indexes.get(&hinge.first) else {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::DisconnectedMaterial,
                },
            ));
        };
        let Some(&second) = face_indexes.get(&hinge.second) else {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::DisconnectedMaterial,
                },
            ));
        };
        let Some(edge) = edges.get(&hinge.edge) else {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::MissingSourceEdge,
                },
            ));
        };
        if !matches!(edge.kind, EdgeKind::Mountain | EdgeKind::Valley) {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::UnassignedHinge,
                },
            ));
        }
        let assignment = match edge.kind {
            EdgeKind::Mountain => FoldAssignment::Mountain,
            EdgeKind::Valley => FoldAssignment::Valley,
            EdgeKind::Auxiliary | EdgeKind::Boundary | EdgeKind::Cut => {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                        reason: UnsupportedFlatFoldabilityTopology::UnassignedHinge,
                    },
                ));
            }
        };
        if assignment != hinge.assignment {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::UnassignedHinge,
                },
            ));
        }
        adjacency[first].push((hinge.edge, second, assignment, edge.clone()));
        adjacency[second].push((hinge.edge, first, assignment, edge.clone()));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable_by_key(|(edge, neighbor, _, _)| {
            (
                source_faces[*neighbor].layer.face_key,
                edge.canonical_bytes(),
            )
        });
    }

    let reference_face = 0;
    let mut transforms = vec![None::<(Transform, bool, Option<EdgeId>)>; source_faces.len()];
    let identity = Transform::identity();
    runtime.add_embedding_exact_storage(exact_storage_bytes_transform(&identity)?)?;
    transforms[reference_face] = Some((identity, true, None));
    let mut queue = VecDeque::from([reference_face]);
    while let Some(face_index) = queue.pop_front() {
        runtime.checkpoint(None)?;
        let (transform, front_up, _) = transforms[face_index].clone().ok_or_else(internal_abort)?;
        let transform_transient_bytes = exact_storage_bytes_transform(&transform)?;
        runtime.ensure_transient_exact_storage(transform_transient_bytes)?;
        for (edge_id, neighbor, _, edge) in &adjacency[face_index] {
            let source_first = vertices.get(&edge.start).ok_or({
                FacewiseAbort::Unknown(GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::MissingSourceVertex,
                })
            })?;
            let source_second = vertices.get(&edge.end).ok_or({
                FacewiseAbort::Unknown(GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::MissingSourceVertex,
                })
            })?;
            let folded_first = apply(&transform, source_first, runtime)?;
            let folded_first_bytes = exact_storage_bytes_point(&folded_first)?;
            runtime.ensure_transient_exact_storage(
                transform_transient_bytes.saturating_add(folded_first_bytes),
            )?;
            let folded_second = apply(&transform, source_second, runtime)?;
            let folded_axis_bytes =
                folded_first_bytes.saturating_add(exact_storage_bytes_point(&folded_second)?);
            runtime.ensure_transient_exact_storage(
                transform_transient_bytes.saturating_add(folded_axis_bytes),
            )?;
            let reflection = reflection_across(&folded_first, &folded_second, runtime)?;
            let reflection_bytes = exact_storage_bytes_transform(&reflection)?;
            let transient_before_candidate = transform_transient_bytes
                .saturating_add(folded_axis_bytes)
                .saturating_add(reflection_bytes);
            runtime.ensure_transient_exact_storage(transient_before_candidate)?;
            let candidate = compose(&reflection, &transform, runtime)?;
            let candidate_bytes = exact_storage_bytes_transform(&candidate)?;
            runtime.ensure_transient_exact_storage(
                transient_before_candidate.saturating_add(candidate_bytes),
            )?;
            let candidate_front_up = !front_up;
            if let Some((existing, existing_front_up, _)) = &transforms[*neighbor] {
                if *existing_front_up != candidate_front_up {
                    return Err(embedding_contradiction(
                        &source_faces[*neighbor],
                        *edge_id,
                        source_faces[*neighbor].vertex_ids[0],
                    ));
                }
                for (vertex_id, point) in source_faces[*neighbor]
                    .vertex_ids
                    .iter()
                    .zip(&source_faces[*neighbor].source_polygon)
                {
                    let existing_point = apply(existing, point, runtime)?;
                    let existing_point_bytes = exact_storage_bytes_point(&existing_point)?;
                    runtime.ensure_transient_exact_storage(
                        transient_before_candidate
                            .saturating_add(candidate_bytes)
                            .saturating_add(existing_point_bytes),
                    )?;
                    let candidate_point = apply(&candidate, point, runtime)?;
                    let comparison_point_bytes = existing_point_bytes
                        .saturating_add(exact_storage_bytes_point(&candidate_point)?);
                    runtime.ensure_transient_exact_storage(
                        transient_before_candidate
                            .saturating_add(candidate_bytes)
                            .saturating_add(comparison_point_bytes),
                    )?;
                    if existing_point != candidate_point {
                        return Err(embedding_contradiction(
                            &source_faces[*neighbor],
                            *edge_id,
                            *vertex_id,
                        ));
                    }
                }
            } else {
                runtime.add_embedding_exact_storage(candidate_bytes)?;
                runtime.ensure_transient_exact_storage(transient_before_candidate)?;
                transforms[*neighbor] = Some((candidate, candidate_front_up, Some(*edge_id)));
                queue.push_back(*neighbor);
            }
        }
    }
    if transforms.iter().any(Option::is_none) {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                reason: UnsupportedFlatFoldabilityTopology::DisconnectedMaterial,
            },
        ));
    }

    let mut folded_faces = Vec::with_capacity(source_faces.len());
    for (source, transform) in source_faces.into_iter().zip(transforms) {
        runtime.checkpoint(None)?;
        let (transform, front_up, _) = transform.ok_or_else(internal_abort)?;
        let mut polygon = Vec::with_capacity(source.source_polygon.len());
        for point in &source.source_polygon {
            let folded_point = apply(&transform, point, runtime)?;
            runtime.add_embedding_exact_storage(exact_storage_bytes_point(&folded_point)?)?;
            polygon.push(folded_point);
        }
        let area = signed_double_area(&polygon, runtime)?;
        if area.is_negative() {
            polygon.reverse();
        } else if area.is_zero() {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::NonSimpleFace,
                },
            ));
        }
        folded_faces.push(FoldedFace {
            source,
            transform,
            front_up,
            polygon,
        });
    }

    let mut folded_hinges = Vec::with_capacity(topology.hinge_adjacency.len());
    for hinge in &topology.hinge_adjacency {
        runtime.checkpoint(None)?;
        let first_face = face_indexes[&hinge.first];
        let second_face = face_indexes[&hinge.second];
        let edge = &edges[&hinge.edge];
        let source_first = &vertices[&edge.start];
        let source_second = &vertices[&edge.end];
        let first_point = apply(&folded_faces[first_face].transform, source_first, runtime)?;
        let first_point_bytes = exact_storage_bytes_point(&first_point)?;
        runtime.ensure_transient_exact_storage(first_point_bytes)?;
        let second_point = apply(&folded_faces[first_face].transform, source_second, runtime)?;
        let stored_hinge_bytes =
            first_point_bytes.saturating_add(exact_storage_bytes_point(&second_point)?);
        runtime.ensure_transient_exact_storage(stored_hinge_bytes)?;
        let other_first = apply(&folded_faces[second_face].transform, source_first, runtime)?;
        let other_first_bytes = exact_storage_bytes_point(&other_first)?;
        runtime
            .ensure_transient_exact_storage(stored_hinge_bytes.saturating_add(other_first_bytes))?;
        let other_second = apply(&folded_faces[second_face].transform, source_second, runtime)?;
        let comparison_bytes =
            other_first_bytes.saturating_add(exact_storage_bytes_point(&other_second)?);
        runtime
            .ensure_transient_exact_storage(stored_hinge_bytes.saturating_add(comparison_bytes))?;
        if first_point != other_first || second_point != other_second {
            return Err(embedding_contradiction(
                &folded_faces[second_face].source,
                hinge.edge,
                edge.start,
            ));
        }
        runtime.add_embedding_exact_storage(stored_hinge_bytes)?;
        runtime.ensure_transient_exact_storage(comparison_bytes)?;
        folded_hinges.push(FoldedHinge {
            edge: hinge.edge,
            first_face,
            second_face,
            assignment: hinge.assignment,
            first_point,
            second_point,
        });
    }
    folded_hinges.sort_unstable_by_key(|hinge| hinge.edge.canonical_bytes());
    let embedding = FlatEmbedding {
        reference_face,
        faces: folded_faces,
        hinges: folded_hinges,
        material_internal_edge_count,
    };
    runtime.set_embedding_exact_storage(exact_storage_bytes_embedding(&embedding)?)?;
    Ok(embedding)
}

fn validate_convex_face<O: GlobalFlatFoldabilityObserver + ?Sized>(
    face: LayerFace,
    polygon: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    let area = signed_double_area(polygon, runtime)?;
    if area <= Rational::zero() {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::NonConvexFace { face },
        ));
    }
    let mut unique_points = HashSet::with_capacity(polygon.len());
    for point in polygon {
        if !unique_points.insert(point) {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                    reason: UnsupportedFlatFoldabilityTopology::NonSimpleFace,
                },
            ));
        }
    }
    for index in 0..polygon.len() {
        let turn = cross(
            &polygon[index],
            &polygon[(index + 1) % polygon.len()],
            &polygon[(index + 2) % polygon.len()],
            runtime,
        )?;
        if turn.is_negative() {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::NonConvexFace { face },
            ));
        }
    }
    Ok(())
}

fn embedding_contradiction(face: &SourceFace, edge: EdgeId, vertex: VertexId) -> FacewiseAbort {
    FacewiseAbort::Impossible(
        GlobalFlatFoldabilityImpossibleReason::InconsistentFlatEmbedding {
            face: face.layer,
            conflicting_hinge: edge,
            conflicting_vertex: vertex,
        },
    )
}

fn internal_abort() -> FacewiseAbort {
    FacewiseAbort::Execution(GlobalFlatFoldabilityExecutionError::Internal {
        reason: GlobalFlatFoldabilityInternalError::ValidatedTopologyInvariantLost,
    })
}

#[derive(Clone)]
struct OverlapPair {
    first: usize,
    second: usize,
}

fn build_overlap_pairs<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[FoldedFace],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<OverlapPair>> {
    let mut pairs = Vec::new();
    for first in 0..faces.len() {
        runtime.checkpoint(None)?;
        for second in (first + 1)..faces.len() {
            let intersection = convex_polygon_intersection(
                &faces[first].polygon,
                &faces[second].polygon,
                runtime,
            )?;
            if intersection.len() >= 3 {
                let area = signed_double_area(&intersection, runtime)?;
                if area.is_positive() {
                    let observed = pairs.len().checked_add(1).ok_or({
                        FacewiseAbort::Unknown(
                            GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                                resource: FlatFoldabilityResource::OverlapFacePairs,
                                limit: runtime.limits.max_overlap_face_pairs,
                                observed: usize::MAX,
                            },
                        )
                    })?;
                    runtime.set_overlap_pairs(observed)?;
                    pairs.push(OverlapPair { first, second });
                }
            }
        }
    }
    Ok(pairs)
}

fn convex_polygon_intersection<O: GlobalFlatFoldabilityObserver + ?Sized>(
    first: &[Point],
    second: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<Point>> {
    let initial_bytes = exact_storage_bytes_points(first)?;
    runtime.ensure_transient_exact_storage(initial_bytes)?;
    let mut output = first.to_vec();
    for edge_index in 0..second.len() {
        if output.is_empty() {
            break;
        }
        let clip_first = &second[edge_index];
        let clip_second = &second[(edge_index + 1) % second.len()];
        let input = std::mem::take(&mut output);
        let input_bytes = exact_storage_bytes_points(&input)?;
        let mut output_bytes = 0_usize;
        let mut previous = input.last().ok_or_else(internal_abort)?;
        let mut previous_side = cross(clip_first, clip_second, previous, runtime)?;
        for current in &input {
            let current_side = cross(clip_first, clip_second, current, runtime)?;
            let previous_inside = !previous_side.is_negative();
            let current_inside = !current_side.is_negative();
            if previous_inside != current_inside {
                let denominator = sub(&previous_side, &current_side, runtime)?;
                let parameter = div(&previous_side, &denominator, runtime)?;
                let intersection = interpolate(previous, current, &parameter, runtime)?;
                push_exact_point_bounded(
                    &mut output,
                    intersection,
                    &mut output_bytes,
                    input_bytes,
                    runtime,
                )?;
            }
            if current_inside {
                push_exact_point_bounded(
                    &mut output,
                    current.clone(),
                    &mut output_bytes,
                    input_bytes,
                    runtime,
                )?;
            }
            previous = current;
            previous_side = current_side;
        }
        deduplicate_polygon(&mut output);
    }
    if output.len() >= 3 && signed_double_area(&output, runtime)?.is_negative() {
        output.reverse();
    }
    Ok(output)
}

fn push_exact_point_bounded<O: GlobalFlatFoldabilityObserver + ?Sized>(
    target: &mut Vec<Point>,
    point: Point,
    target_bytes: &mut usize,
    other_transient_bytes: usize,
    runtime: &Runtime<'_, O>,
) -> FacewiseResult<()> {
    let point_bytes = exact_storage_bytes_point(&point)?;
    let next_target_bytes = target_bytes.saturating_add(point_bytes);
    runtime
        .ensure_transient_exact_storage(other_transient_bytes.saturating_add(next_target_bytes))?;
    target.push(point);
    *target_bytes = next_target_bytes;
    Ok(())
}

fn interpolate<O: GlobalFlatFoldabilityObserver + ?Sized>(
    first: &Point,
    second: &Point,
    parameter: &Rational,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Point> {
    let dx = sub(&second.x, &first.x, runtime)?;
    let dy = sub(&second.y, &first.y, runtime)?;
    Ok(Point {
        x: add(&first.x, &mul(parameter, &dx, runtime)?, runtime)?,
        y: add(&first.y, &mul(parameter, &dy, runtime)?, runtime)?,
    })
}

fn deduplicate_polygon(polygon: &mut Vec<Point>) {
    polygon.dedup();
    if polygon.len() > 1 && polygon.first() == polygon.last() {
        polygon.pop();
    }
}

#[derive(Clone)]
struct OverlapCell {
    key: OverlapCellKey,
    boundary: Vec<Point>,
    covering_faces: Vec<usize>,
}

fn build_overlap_cells<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[FoldedFace],
    pairs: &[OverlapPair],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<OverlapCell>> {
    let mut all_points = faces.iter().flat_map(|face| face.polygon.iter());
    let first = all_points.next().ok_or_else(internal_abort)?;
    let (mut min_x, mut max_x) = (first.x.clone(), first.x.clone());
    let (mut min_y, mut max_y) = (first.y.clone(), first.y.clone());
    for point in all_points {
        if cmp(&point.x, &min_x, runtime)? == Ordering::Less {
            min_x = point.x.clone();
        }
        if cmp(&point.x, &max_x, runtime)? == Ordering::Greater {
            max_x = point.x.clone();
        }
        if cmp(&point.y, &min_y, runtime)? == Ordering::Less {
            min_y = point.y.clone();
        }
        if cmp(&point.y, &max_y, runtime)? == Ordering::Greater {
            max_y = point.y.clone();
        }
    }
    if min_x == max_x || min_y == max_y {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::UnsupportedTopology {
                reason: UnsupportedFlatFoldabilityTopology::NonSimpleFace,
            },
        ));
    }
    let bounding_rectangle = vec![
        Point {
            x: min_x.clone(),
            y: min_y.clone(),
        },
        Point {
            x: max_x.clone(),
            y: min_y,
        },
        Point {
            x: max_x,
            y: max_y.clone(),
        },
        Point { x: min_x, y: max_y },
    ];
    runtime.set_arrangement_exact_storage(exact_storage_bytes_points(&bounding_rectangle)?)?;
    let mut regions = vec![bounding_rectangle];
    let mut supporting_lines = faces
        .iter()
        .enumerate()
        .flat_map(|(face_index, face)| {
            (0..face.polygon.len()).map(move |edge_index| (face_index, edge_index))
        })
        .collect::<Vec<_>>();
    supporting_lines.sort_unstable_by_key(|(face_index, edge_index)| {
        (
            faces[*face_index].source.layer.face_key,
            *edge_index,
            faces[*face_index].source.layer.face_id.canonical_bytes(),
        )
    });
    for (face_index, edge_index) in supporting_lines {
        runtime.checkpoint(None)?;
        let line_first = &faces[face_index].polygon[edge_index];
        let line_second =
            &faces[face_index].polygon[(edge_index + 1) % faces[face_index].polygon.len()];
        let mut next_regions = Vec::new();
        let mut next_region_bytes = 0_usize;
        for region in regions {
            runtime.checkpoint(None)?;
            let mut left = clip_polygon_halfplane(
                &region,
                line_first,
                line_second,
                true,
                next_region_bytes,
                runtime,
            )?;
            let left_bytes = exact_storage_bytes_points(&left)?;
            let mut right = clip_polygon_halfplane(
                &region,
                line_first,
                line_second,
                false,
                next_region_bytes.saturating_add(left_bytes),
                runtime,
            )?;
            simplify_convex_polygon(&mut left, runtime)?;
            simplify_convex_polygon(&mut right, runtime)?;
            for candidate in [left, right] {
                if candidate.len() < 3 {
                    continue;
                }
                if signed_double_area(&candidate, runtime)?.is_positive() {
                    let candidate_bytes = exact_storage_bytes_points(&candidate)?;
                    next_region_bytes = next_region_bytes.saturating_add(candidate_bytes);
                    runtime.ensure_transient_exact_storage(next_region_bytes)?;
                    next_regions.push(candidate);
                }
            }
        }
        if next_regions.len() > runtime.limits.max_overlap_cells {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::OverlapArrangementLimitReached {
                    resource: FlatFoldabilityResource::OverlapCells,
                    limit: runtime.limits.max_overlap_cells,
                    observed: next_regions.len(),
                },
            ));
        }
        let segment_count = next_regions.iter().try_fold(0_usize, |total, region| {
            total.checked_add(region.len()).ok_or({
                FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::OverlapArrangementLimitReached {
                        resource: FlatFoldabilityResource::ArrangementSegments,
                        limit: runtime.limits.max_arrangement_segments,
                        observed: usize::MAX,
                    },
                )
            })
        })?;
        runtime.set_arrangement_segments(segment_count)?;
        runtime.set_arrangement_exact_storage(next_region_bytes)?;
        regions = next_regions;
    }

    let mut cells = BTreeMap::<[u8; 32], OverlapCell>::new();
    for region in regions {
        runtime.checkpoint(None)?;
        let representative = representative_point(&region, runtime)?;
        let mut covering_faces = Vec::new();
        let mut covering_face_poll = 0_usize;
        for (index, face) in faces.iter().enumerate() {
            runtime.poll_control(&mut covering_face_poll)?;
            if point_in_convex_polygon(&representative, &face.polygon, runtime)? {
                if covering_faces.len() == covering_faces.capacity() {
                    let prior_capacity = covering_faces.capacity();
                    let next_capacity = next_vector_capacity(
                        prior_capacity,
                        covering_faces.len(),
                        faces.len(),
                        runtime,
                    )?;
                    let old_bytes =
                        runtime.allocation_bytes(prior_capacity, std::mem::size_of::<usize>())?;
                    let next_bytes =
                        runtime.allocation_bytes(next_capacity, std::mem::size_of::<usize>())?;
                    runtime.ensure_transient_exact_storage(
                        old_bytes
                            .checked_add(next_bytes)
                            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
                    )?;
                    covering_faces
                        .try_reserve_exact(next_capacity - covering_faces.len())
                        .map_err(|_| {
                            runtime
                                .exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
                        })?;
                }
                covering_faces.push(index);
            }
        }
        drop(representative);
        if covering_faces.is_empty() {
            continue;
        }
        let key = overlap_cell_key(&region, &covering_faces, faces, runtime)?;
        if let std::collections::btree_map::Entry::Vacant(entry) = cells.entry(key.0) {
            let covering_bytes = runtime
                .allocation_bytes(covering_faces.capacity(), std::mem::size_of::<usize>())?;
            let retained_structure_bytes = std::mem::size_of::<OverlapCell>()
                .checked_add(std::mem::size_of::<[u8; 32]>())
                .and_then(|total| total.checked_add(covering_bytes))
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
            runtime.add_certificate_structure_storage(retained_structure_bytes)?;
            entry.insert(OverlapCell {
                key,
                boundary: region,
                covering_faces,
            });
        }
        runtime.set_overlap_cells(cells.len())?;
    }
    let cells = cells.into_values().collect::<Vec<_>>();
    let cell_boundary_bytes = cells.iter().try_fold(0_usize, |total, cell| {
        Ok::<_, ExactError>(total.saturating_add(exact_storage_bytes_points(&cell.boundary)?))
    })?;
    runtime.set_arrangement_exact_storage(cell_boundary_bytes)?;
    for pair in pairs {
        if !cells.iter().any(|cell| {
            cell.covering_faces.contains(&pair.first) && cell.covering_faces.contains(&pair.second)
        }) {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
                },
            ));
        }
    }
    Ok(cells)
}

fn clip_polygon_halfplane<O: GlobalFlatFoldabilityObserver + ?Sized>(
    polygon: &[Point],
    line_first: &Point,
    line_second: &Point,
    keep_left: bool,
    other_transient_bytes: usize,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<Point>> {
    if polygon.is_empty() {
        return Ok(Vec::new());
    }
    let mut output = Vec::new();
    let mut output_bytes = 0_usize;
    let mut previous = polygon.last().ok_or_else(internal_abort)?;
    let mut previous_side = cross(line_first, line_second, previous, runtime)?;
    for current in polygon {
        let current_side = cross(line_first, line_second, current, runtime)?;
        let previous_inside = if keep_left {
            !previous_side.is_negative()
        } else {
            !previous_side.is_positive()
        };
        let current_inside = if keep_left {
            !current_side.is_negative()
        } else {
            !current_side.is_positive()
        };
        if previous_inside != current_inside {
            let denominator = sub(&previous_side, &current_side, runtime)?;
            let parameter = div(&previous_side, &denominator, runtime)?;
            let intersection = interpolate(previous, current, &parameter, runtime)?;
            push_exact_point_bounded(
                &mut output,
                intersection,
                &mut output_bytes,
                other_transient_bytes,
                runtime,
            )?;
        }
        if current_inside {
            push_exact_point_bounded(
                &mut output,
                current.clone(),
                &mut output_bytes,
                other_transient_bytes,
                runtime,
            )?;
        }
        previous = current;
        previous_side = current_side;
    }
    deduplicate_polygon(&mut output);
    Ok(output)
}

fn simplify_convex_polygon<O: GlobalFlatFoldabilityObserver + ?Sized>(
    polygon: &mut Vec<Point>,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    let mut changed = true;
    while changed && polygon.len() >= 3 {
        changed = false;
        for index in 0..polygon.len() {
            let previous = (index + polygon.len() - 1) % polygon.len();
            let next = (index + 1) % polygon.len();
            if cross(&polygon[previous], &polygon[index], &polygon[next], runtime)?.is_zero() {
                polygon.remove(index);
                changed = true;
                break;
            }
        }
    }
    Ok(())
}

fn representative_point<O: GlobalFlatFoldabilityObserver + ?Sized>(
    polygon: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Point> {
    if polygon.len() < 3 {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
            },
        ));
    }
    for index in 0..polygon.len() {
        let first = &polygon[(index + polygon.len() - 1) % polygon.len()];
        let second = &polygon[index];
        let third = &polygon[(index + 1) % polygon.len()];
        if cross(first, second, third, runtime)?.is_positive() {
            let candidate = average3(first, second, third, runtime)?;
            if point_in_simple_polygon(&candidate, polygon, runtime)? {
                return Ok(candidate);
            }
        }
    }
    Err(FacewiseAbort::Unknown(
        GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
            reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
        },
    ))
}

fn point_in_convex_polygon<O: GlobalFlatFoldabilityObserver + ?Sized>(
    point: &Point,
    polygon: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    for index in 0..polygon.len() {
        let side = cross(
            &polygon[index],
            &polygon[(index + 1) % polygon.len()],
            point,
            runtime,
        )?;
        if !side.is_positive() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn point_in_simple_polygon<O: GlobalFlatFoldabilityObserver + ?Sized>(
    point: &Point,
    polygon: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    let mut winding = 0_i32;
    for index in 0..polygon.len() {
        let first = &polygon[index];
        let second = &polygon[(index + 1) % polygon.len()];
        let side = cross(first, second, point, runtime)?;
        if side.is_zero() && point_in_segment_bounds(point, first, second, runtime)? {
            return Ok(false);
        }
        let first_below = cmp(&first.y, &point.y, runtime)? != Ordering::Greater;
        let second_below = cmp(&second.y, &point.y, runtime)? != Ordering::Greater;
        if first_below && !second_below && side.is_positive() {
            winding += 1;
        } else if !first_below && second_below && side.is_negative() {
            winding -= 1;
        }
    }
    Ok(winding != 0)
}

fn point_in_segment_bounds<O: GlobalFlatFoldabilityObserver + ?Sized>(
    point: &Point,
    first: &Point,
    second: &Point,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    let min_x = if cmp(&first.x, &second.x, runtime)? == Ordering::Greater {
        &second.x
    } else {
        &first.x
    };
    let max_x = if cmp(&first.x, &second.x, runtime)? == Ordering::Greater {
        &first.x
    } else {
        &second.x
    };
    let min_y = if cmp(&first.y, &second.y, runtime)? == Ordering::Greater {
        &second.y
    } else {
        &first.y
    };
    let max_y = if cmp(&first.y, &second.y, runtime)? == Ordering::Greater {
        &first.y
    } else {
        &second.y
    };
    Ok(cmp(&point.x, min_x, runtime)? != Ordering::Less
        && cmp(&point.x, max_x, runtime)? != Ordering::Greater
        && cmp(&point.y, min_y, runtime)? != Ordering::Less
        && cmp(&point.y, max_y, runtime)? != Ordering::Greater)
}

fn overlap_cell_key<O: GlobalFlatFoldabilityObserver + ?Sized>(
    boundary: &[Point],
    covering_faces: &[usize],
    faces: &[FoldedFace],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<OverlapCellKey> {
    runtime.checkpoint(None)?;
    let encoded_headers =
        runtime.allocation_bytes(boundary.len(), std::mem::size_of::<Vec<u8>>())?;
    let canonical_transient = exact_storage_bytes_points(boundary)?
        .checked_add(encoded_headers)
        .and_then(|total| total.checked_add(std::mem::size_of::<Vec<Vec<u8>>>()))
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    runtime.ensure_transient_exact_storage(canonical_transient)?;
    let canonical = canonical_boundary_bytes(boundary, runtime)?;
    let mut hasher = Sha256::new();
    hasher.update(CELL_KEY_DOMAIN);
    hasher.update(
        u64::try_from(boundary.len())
            .map_err(|_| FacewiseAbort::Execution(internal_error()))?
            .to_be_bytes(),
    );
    let mut hash_poll = 0_usize;
    for point in canonical {
        runtime.poll_control(&mut hash_poll)?;
        hasher.update(
            u64::try_from(point.len())
                .map_err(|_| FacewiseAbort::Execution(internal_error()))?
                .to_be_bytes(),
        );
        hasher.update(point);
    }
    for face in covering_faces {
        runtime.poll_control(&mut hash_poll)?;
        hasher.update(faces[*face].source.layer.face_key.0);
    }
    runtime.checkpoint(None)?;
    Ok(OverlapCellKey(hasher.finalize().into()))
}

fn canonical_boundary_bytes<O: GlobalFlatFoldabilityObserver + ?Sized>(
    boundary: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<Vec<u8>>> {
    let mut encoded = Vec::with_capacity(boundary.len());
    let mut control_poll = 0_usize;
    for point in boundary {
        runtime.poll_control(&mut control_poll)?;
        let mut bytes = rational_bytes(&point.x)?;
        bytes.extend_from_slice(&rational_bytes(&point.y)?);
        encoded.push(bytes);
    }
    let Some((mut start, first)) = encoded.iter().enumerate().next() else {
        return Err(FacewiseAbort::Execution(internal_error()));
    };
    let mut minimum = first;
    for (index, candidate) in encoded.iter().enumerate().skip(1) {
        runtime.poll_control(&mut control_poll)?;
        if candidate < minimum {
            start = index;
            minimum = candidate;
        }
    }
    let len = encoded.len();
    let mut forward_vs_reverse = Ordering::Equal;
    for offset in 0..len {
        runtime.poll_control(&mut control_poll)?;
        let ordering = encoded[(start + offset) % len].cmp(&encoded[(start + len - offset) % len]);
        if ordering != Ordering::Equal {
            forward_vs_reverse = ordering;
            break;
        }
    }
    let reverse = forward_vs_reverse == Ordering::Greater;
    let mut canonical = Vec::with_capacity(len);
    for offset in 0..len {
        runtime.poll_control(&mut control_poll)?;
        let index = if reverse {
            (start + len - offset) % len
        } else {
            (start + offset) % len
        };
        canonical.push(std::mem::take(&mut encoded[index]));
    }
    runtime.checkpoint(None)?;
    Ok(canonical)
}

fn internal_error() -> GlobalFlatFoldabilityExecutionError {
    GlobalFlatFoldabilityExecutionError::Internal {
        reason: GlobalFlatFoldabilityInternalError::ValidatedTopologyInvariantLost,
    }
}

fn next_vector_capacity<O: GlobalFlatFoldabilityObserver + ?Sized>(
    current_capacity: usize,
    current_len: usize,
    maximum_len: usize,
    runtime: &Runtime<'_, O>,
) -> FacewiseResult<usize> {
    let required = current_len
        .checked_add(1)
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    if required > maximum_len {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    let geometric = if current_capacity == 0 {
        4
    } else {
        current_capacity.saturating_mul(2)
    };
    let next = geometric.max(required).min(maximum_len);
    if next < required || next == usize::MAX {
        return Err(runtime.exact_storage_limit_failure(usize::MAX));
    }
    Ok(next)
}

fn layer_order_derivation(
    embedding: &FlatEmbedding,
    reference_face: LayerFace,
    overlap_cell_count: usize,
    constraint_count: usize,
) -> LayerOrderDerivation {
    if embedding.hinges.is_empty() && embedding.faces.len() == 1 {
        LayerOrderDerivation::SingleFace {
            face: reference_face,
        }
    } else if embedding.hinges.len() == 1 && embedding.faces.len() == 2 {
        let hinge = &embedding.hinges[0];
        LayerOrderDerivation::SingleHinge {
            hinge_edge: hinge.edge,
            assignment: hinge.assignment,
            canonical_first: embedding.faces[hinge.first_face].source.layer,
            canonical_second: embedding.faces[hinge.second_face].source.layer,
        }
    } else {
        LayerOrderDerivation::FacewiseCertificate {
            reference_face,
            overlap_cell_count,
            constraint_count,
        }
    }
}

fn solve_layer_order<O: GlobalFlatFoldabilityObserver + ?Sized>(
    embedding: FlatEmbedding,
    pairs: Vec<OverlapPair>,
    cells: Vec<OverlapCell>,
    provenance: GlobalFlatFoldabilityProvenance,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<SolveSuccess> {
    runtime.advance(GlobalFlatFoldabilityPhase::BuildingConstraints, None)?;
    let problem = build_constraint_problem(&embedding, &pairs, &cells, runtime, true)?;
    if problem.variables.len() != runtime.work.overlap_face_pairs
        || problem.constraints.len() != runtime.work.constraints
    {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    runtime.advance(GlobalFlatFoldabilityPhase::Propagating, None)?;
    let solver_memory_limit = runtime.remaining_storage_bytes()?;
    let solver_result = solve_constraints_with_memory(
        problem.variables.len(),
        &problem.constraints,
        &problem.fixed_assignments,
        runtime.limits.max_search_nodes,
        solver_memory_limit,
        |event, search_nodes| runtime.constraint_solver_control(event, search_nodes),
    );
    let assignment = match solver_result {
        ConstraintSolverResult::Satisfied {
            assignment,
            search_nodes,
        } => {
            runtime.set_search_nodes(search_nodes)?;
            runtime.add_constraint_storage(
                runtime.allocation_bytes(assignment.len(), std::mem::size_of::<bool>())?,
            )?;
            assignment
        }
        ConstraintSolverResult::Unsatisfied {
            conflict_constraint,
            search_nodes,
        } => {
            runtime.set_search_nodes(search_nodes)?;
            if let Some(index) = conflict_constraint {
                let constraint = problem.constraints.get(index).ok_or_else(internal_abort)?;
                return Err(constraint_contradiction(constraint, &embedding));
            }
            return Err(FacewiseAbort::Impossible(
                GlobalFlatFoldabilityImpossibleReason::FacewiseSearchExhausted {
                    variable_count: problem.variables.len(),
                    constraint_count: problem.constraints.len(),
                },
            ));
        }
        ConstraintSolverResult::SearchNodeLimit { observed } => {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::SearchNodes,
                    limit: runtime.limits.max_search_nodes,
                    observed,
                },
            ));
        }
        ConstraintSolverResult::DeadlineReached { search_nodes } => {
            runtime.set_search_nodes(search_nodes)?;
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                    phase: runtime.phase,
                },
            ));
        }
        ConstraintSolverResult::Cancelled => {
            return Err(FacewiseAbort::Execution(
                GlobalFlatFoldabilityExecutionError::Cancelled,
            ));
        }
        ConstraintSolverResult::WorkingMemoryLimit { observed } => {
            let used = runtime
                .exact_storage
                .total()
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
            let total_observed = used.saturating_add(observed);
            return Err(runtime.exact_storage_limit_failure(total_observed));
        }
        ConstraintSolverResult::InvalidConstraint => {
            return Err(FacewiseAbort::Execution(internal_error()));
        }
    };
    if runtime.phase < GlobalFlatFoldabilityPhase::VerifyingCertificate {
        runtime.advance(
            GlobalFlatFoldabilityPhase::VerifyingCertificate,
            Some(problem.constraints.len()),
        )?;
    }
    verify_facewise_certificate(&embedding, &pairs, &cells, &problem, &assignment, runtime)?;
    runtime.add_certificate_structure_storage(runtime.allocation_bytes(
        problem.variables.len(),
        std::mem::size_of::<((usize, usize), bool)>(),
    )?)?;
    let pair_values = PairValues::try_from_parallel(&problem.variables, &assignment)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    drop(assignment);
    drop(problem);
    runtime.clear_constraint_storage();

    let reference_face = embedding.faces[embedding.reference_face].source.layer;
    let face_pair_order_bytes = runtime.allocation_bytes(
        pair_values.len(),
        std::mem::size_of::<FacePairOrderSnapshot>(),
    )?;
    runtime.add_certificate_structure_storage(face_pair_order_bytes)?;
    let mut face_pair_orders = Vec::new();
    face_pair_orders
        .try_reserve_exact(pair_values.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for &((first, second), second_above_first) in pair_values.iter() {
        let (lower, upper) = if second_above_first {
            (first, second)
        } else {
            (second, first)
        };
        let mut supporting_cells = Vec::new();
        let mut supporting_cell_poll = 0_usize;
        for cell in &cells {
            runtime.poll_control(&mut supporting_cell_poll)?;
            if cell.covering_faces.contains(&first) && cell.covering_faces.contains(&second) {
                if supporting_cells.len() == supporting_cells.capacity() {
                    let prior_capacity = supporting_cells.capacity();
                    let next_capacity = next_vector_capacity(
                        prior_capacity,
                        supporting_cells.len(),
                        cells.len(),
                        runtime,
                    )?;
                    let next_bytes = runtime
                        .allocation_bytes(next_capacity, std::mem::size_of::<OverlapCellKey>())?;
                    runtime.ensure_transient_exact_storage(next_bytes)?;
                    runtime.add_certificate_structure_storage(runtime.allocation_bytes(
                        next_capacity - prior_capacity,
                        std::mem::size_of::<OverlapCellKey>(),
                    )?)?;
                    supporting_cells
                        .try_reserve_exact(next_capacity - supporting_cells.len())
                        .map_err(|_| {
                            runtime
                                .exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
                        })?;
                }
                supporting_cells.push(cell.key);
            }
        }
        runtime.checkpoint(None)?;
        supporting_cells.sort_unstable_by_key(|key| key.0);
        face_pair_orders.push(FacePairOrderSnapshot {
            lower_face: embedding.faces[lower].source.layer,
            upper_face: embedding.faces[upper].source.layer,
            supporting_cells,
        });
    }
    face_pair_orders.sort_unstable_by_key(|order| {
        (
            order.lower_face.face_key,
            order.upper_face.face_key,
            order.lower_face.face_id.canonical_bytes(),
            order.upper_face.face_id.canonical_bytes(),
        )
    });

    let overlap_cell_snapshot_bytes =
        runtime.allocation_bytes(cells.len(), std::mem::size_of::<OverlapCellSnapshot>())?;
    runtime.add_certificate_structure_storage(overlap_cell_snapshot_bytes)?;
    let mut overlap_cells = Vec::new();
    overlap_cells
        .try_reserve_exact(cells.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let mut maximum_ply = 1_usize;
    for cell in &cells {
        runtime.checkpoint(None)?;
        let boundary_structure_bytes = runtime.allocation_bytes(
            cell.boundary.len(),
            std::mem::size_of::<crate::ExactPointValue>(),
        )?;
        let covering_structure_bytes = runtime
            .allocation_bytes(cell.covering_faces.len(), std::mem::size_of::<LayerFace>())?;
        let ordered_structure_bytes = runtime.allocation_bytes(
            cell.covering_faces.len(),
            std::mem::size_of::<ori_domain::FaceId>(),
        )?;
        let inner_structure_bytes = boundary_structure_bytes
            .checked_add(covering_structure_bytes)
            .and_then(|total| total.checked_add(ordered_structure_bytes))
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
        runtime.add_certificate_structure_storage(inner_structure_bytes)?;
        let ordered = order_cell_faces(&cell.covering_faces, &pair_values, runtime)?;
        runtime.ensure_transient_exact_storage(
            runtime.allocation_bytes(ordered.capacity(), std::mem::size_of::<usize>())?,
        )?;
        maximum_ply = maximum_ply.max(ordered.len());
        runtime.add_snapshot_exact_storage(exact_storage_bytes_points(&cell.boundary)?)?;
        let mut exact_boundary = Vec::new();
        exact_boundary
            .try_reserve_exact(cell.boundary.len())
            .map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
        for point in &cell.boundary {
            exact_boundary.push(point.to_value());
        }
        let mut covering_face_snapshots = Vec::new();
        covering_face_snapshots
            .try_reserve_exact(cell.covering_faces.len())
            .map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
        for index in &cell.covering_faces {
            covering_face_snapshots.push(embedding.faces[*index].source.layer);
        }
        let mut bottom_to_top_faces = Vec::new();
        bottom_to_top_faces
            .try_reserve_exact(ordered.len())
            .map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
        for index in &ordered {
            bottom_to_top_faces.push(embedding.faces[*index].source.layer.face_id);
        }
        overlap_cells.push(OverlapCellSnapshot {
            cell_key: cell.key,
            exact_boundary,
            covering_faces: covering_face_snapshots,
            bottom_to_top_faces,
        });
    }
    overlap_cells.sort_unstable_by_key(|cell| cell.cell_key.0);
    let global_order =
        canonical_global_linear_extension(embedding.faces.len(), &pair_values, runtime)?;
    let global_bottom_to_top = if let Some(order) = global_order {
        let final_bytes =
            runtime.allocation_bytes(order.len(), std::mem::size_of::<LayerFace>())?;
        let order_bytes =
            runtime.allocation_bytes(order.capacity(), std::mem::size_of::<usize>())?;
        runtime.ensure_transient_exact_storage(
            final_bytes
                .checked_add(order_bytes)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        runtime.add_certificate_structure_storage(final_bytes)?;
        let mut faces = Vec::new();
        faces.try_reserve_exact(order.len()).map_err(|_| {
            runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
        })?;
        for index in order {
            faces.push(embedding.faces[index].source.layer);
        }
        Some(faces)
    } else {
        None
    };
    runtime.add_certificate_structure_storage(
        runtime.allocation_bytes(embedding.faces.len(), std::mem::size_of::<LayerFace>())?,
    )?;
    let mut material_faces = Vec::new();
    material_faces
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for face in &embedding.faces {
        material_faces.push(face.source.layer);
    }
    runtime.add_certificate_structure_storage(runtime.allocation_bytes(
        embedding.faces.len(),
        std::mem::size_of::<FoldedFaceSnapshot>(),
    )?)?;
    let mut folded_faces = Vec::new();
    folded_faces
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for face in &embedding.faces {
        runtime.add_snapshot_exact_storage(exact_storage_bytes_transform(&face.transform)?)?;
        folded_faces.push(FoldedFaceSnapshot {
            face: face.source.layer,
            source_to_flat: face.transform.to_value(),
            orientation: if face.front_up {
                FoldedFaceOrientation::FrontUp
            } else {
                FoldedFaceOrientation::BackUp
            },
        });
    }
    let proof_summary = FacewiseProofSummary {
        material_faces: embedding.faces.len(),
        overlap_face_pairs: pair_values.len(),
        overlap_cells: overlap_cells.len(),
        constraints: runtime.work.constraints,
        search_nodes: runtime.work.search_nodes,
        maximum_ply,
        certificate_bytes: 0,
    };
    let derivation = layer_order_derivation(
        &embedding,
        reference_face,
        overlap_cells.len(),
        runtime.work.constraints,
    );
    let mut layer_order = LayerOrderSnapshot {
        model_id: LayerOrderModelId::FacewiseLayerOrderV1,
        material_faces,
        global_bottom_to_top,
        provenance: LayerOrderProvenance {
            source: provenance,
            derivation,
        },
        reference_face: Some(reference_face),
        folded_faces,
        overlap_cells,
        face_pair_orders,
        proof_summary: Some(proof_summary),
    };
    finalize_certificate_size(&mut layer_order, runtime)?;
    verify_layer_order_snapshot(
        &layer_order,
        &embedding,
        &cells,
        &pair_values,
        provenance,
        runtime,
    )?;
    Ok(SolveSuccess {
        reason: GlobalFlatFoldabilityPossibleReason::FacewiseConstraintCertificate {
            reference_face,
            overlap_cell_count: layer_order.overlap_cells.len(),
            constraint_count: runtime.work.constraints,
        },
        layer_order,
    })
}

fn verify_layer_order_snapshot<O: GlobalFlatFoldabilityObserver + ?Sized>(
    layer_order: &LayerOrderSnapshot,
    embedding: &FlatEmbedding,
    cells: &[OverlapCell],
    pair_values: &PairValues,
    provenance: GlobalFlatFoldabilityProvenance,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    runtime.clear_verification_storage();
    runtime.checkpoint(None)?;
    runtime.add_verification_storage(
        runtime.allocation_bytes(embedding.faces.len(), std::mem::size_of::<LayerFace>())?,
    )?;
    let mut expected_material_faces = Vec::new();
    expected_material_faces
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for face in &embedding.faces {
        expected_material_faces.push(face.source.layer);
    }
    let reference_face = embedding.faces[embedding.reference_face].source.layer;
    let expected_derivation = layer_order_derivation(
        embedding,
        reference_face,
        cells.len(),
        runtime.work.constraints,
    );
    if layer_order.model_id != LayerOrderModelId::FacewiseLayerOrderV1
        || layer_order.provenance.source != provenance
        || layer_order.provenance.derivation != expected_derivation
        || layer_order.reference_face != Some(reference_face)
        || layer_order.material_faces != expected_material_faces
        || layer_order.folded_faces.len() != embedding.faces.len()
        || layer_order.overlap_cells.len() != cells.len()
        || layer_order.face_pair_orders.len() != pair_values.len()
    {
        return Err(certificate_failure());
    }
    for (snapshot, face) in layer_order.folded_faces.iter().zip(&embedding.faces) {
        runtime.ensure_transient_exact_storage(exact_storage_bytes_transform(&face.transform)?)?;
        if snapshot.face != face.source.layer
            || snapshot.source_to_flat != face.transform.to_value()
            || snapshot.orientation
                != if face.front_up {
                    FoldedFaceOrientation::FrontUp
                } else {
                    FoldedFaceOrientation::BackUp
                }
        {
            return Err(certificate_failure());
        }
    }

    for cell in cells {
        runtime.checkpoint(None)?;
        if overlap_cell_key(
            &cell.boundary,
            &cell.covering_faces,
            &embedding.faces,
            runtime,
        )? != cell.key
        {
            return Err(certificate_failure());
        }
    }
    runtime.add_verification_storage(
        runtime.allocation_bytes(cells.len(), std::mem::size_of::<OverlapCellKey>())?,
    )?;
    let mut internal_cell_keys = Vec::new();
    internal_cell_keys
        .try_reserve_exact(cells.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for cell in cells {
        internal_cell_keys.push(cell.key);
    }
    internal_cell_keys.sort_unstable_by_key(|key| key.0);
    if internal_cell_keys.windows(2).any(|keys| keys[0] == keys[1]) {
        return Err(certificate_failure());
    }
    runtime.add_verification_storage(runtime.allocation_bytes(
        layer_order.overlap_cells.len(),
        std::mem::size_of::<OverlapCellKey>(),
    )?)?;
    let mut snapshot_cell_keys = Vec::new();
    snapshot_cell_keys
        .try_reserve_exact(layer_order.overlap_cells.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for cell in &layer_order.overlap_cells {
        snapshot_cell_keys.push(cell.cell_key);
    }
    if snapshot_cell_keys != internal_cell_keys {
        return Err(certificate_failure());
    }
    let internal_cell_entry_bytes = std::mem::size_of::<(OverlapCellKey, &OverlapCell)>()
        .checked_add(3 * std::mem::size_of::<usize>())
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    runtime.add_verification_storage(
        runtime.allocation_bytes(cells.len(), internal_cell_entry_bytes)?,
    )?;
    let mut internal_cells = HashMap::new();
    internal_cells
        .try_reserve(cells.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for cell in cells {
        internal_cells.insert(cell.key, cell);
    }
    for snapshot in &layer_order.overlap_cells {
        runtime.checkpoint(None)?;
        let Some(cell) = internal_cells.get(&snapshot.cell_key).copied() else {
            return Err(certificate_failure());
        };
        let expected_order = order_cell_faces(&cell.covering_faces, pair_values, runtime)?;
        let expected_boundary_structure = runtime.allocation_bytes(
            cell.boundary.len(),
            std::mem::size_of::<crate::ExactPointValue>(),
        )?;
        let expected_covering_structure = runtime
            .allocation_bytes(cell.covering_faces.len(), std::mem::size_of::<LayerFace>())?;
        let expected_order_structure = runtime.allocation_bytes(
            expected_order.len(),
            std::mem::size_of::<ori_domain::FaceId>(),
        )?;
        let expected_structure = expected_boundary_structure
            .checked_add(expected_covering_structure)
            .and_then(|total| total.checked_add(expected_order_structure))
            .and_then(|total| {
                total.checked_add(
                    runtime
                        .allocation_bytes(expected_order.capacity(), std::mem::size_of::<usize>())
                        .ok()?,
                )
            })
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
        let expected_exact = exact_storage_bytes_points(&cell.boundary)?;
        runtime.ensure_transient_exact_storage(
            expected_structure
                .checked_add(expected_exact)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        if snapshot.exact_boundary
            != cell
                .boundary
                .iter()
                .map(Point::to_value)
                .collect::<Vec<_>>()
            || snapshot.covering_faces
                != cell
                    .covering_faces
                    .iter()
                    .map(|index| embedding.faces[*index].source.layer)
                    .collect::<Vec<_>>()
            || snapshot.bottom_to_top_faces
                != expected_order
                    .iter()
                    .map(|index| embedding.faces[*index].source.layer.face_id)
                    .collect::<Vec<_>>()
        {
            return Err(certificate_failure());
        }
    }

    runtime.add_verification_storage(runtime.allocation_bytes(
        pair_values.len(),
        std::mem::size_of::<FacePairOrderSnapshot>(),
    )?)?;
    let mut expected_face_pair_orders = Vec::new();
    expected_face_pair_orders
        .try_reserve_exact(pair_values.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for &((first, second), canonical_second_above_first) in pair_values.iter() {
        runtime.checkpoint(None)?;
        let (lower, upper) = if canonical_second_above_first {
            (first, second)
        } else {
            (second, first)
        };
        let mut supporting_cells = Vec::new();
        let mut supporting_cell_poll = 0_usize;
        for cell in cells {
            runtime.poll_control(&mut supporting_cell_poll)?;
            if cell.covering_faces.contains(&first) && cell.covering_faces.contains(&second) {
                if supporting_cells.len() == supporting_cells.capacity() {
                    let prior_capacity = supporting_cells.capacity();
                    let next_capacity = next_vector_capacity(
                        prior_capacity,
                        supporting_cells.len(),
                        cells.len(),
                        runtime,
                    )?;
                    let next_bytes = runtime
                        .allocation_bytes(next_capacity, std::mem::size_of::<OverlapCellKey>())?;
                    runtime.ensure_transient_exact_storage(next_bytes)?;
                    runtime.add_verification_storage(runtime.allocation_bytes(
                        next_capacity - prior_capacity,
                        std::mem::size_of::<OverlapCellKey>(),
                    )?)?;
                    supporting_cells
                        .try_reserve_exact(next_capacity - supporting_cells.len())
                        .map_err(|_| {
                            runtime
                                .exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
                        })?;
                }
                supporting_cells.push(cell.key);
            }
        }
        runtime.checkpoint(None)?;
        supporting_cells.sort_unstable_by_key(|key| key.0);
        if supporting_cells.is_empty() {
            return Err(certificate_failure());
        }
        expected_face_pair_orders.push(FacePairOrderSnapshot {
            lower_face: embedding.faces[lower].source.layer,
            upper_face: embedding.faces[upper].source.layer,
            supporting_cells,
        });
    }
    expected_face_pair_orders.sort_unstable_by_key(|order| {
        (
            order.lower_face.face_key,
            order.upper_face.face_key,
            order.lower_face.face_id.canonical_bytes(),
            order.upper_face.face_id.canonical_bytes(),
        )
    });
    if layer_order.face_pair_orders != expected_face_pair_orders {
        return Err(certificate_failure());
    }

    let recomputed_order =
        canonical_global_linear_extension(embedding.faces.len(), pair_values, runtime)?;
    let recomputed_global = if let Some(order) = recomputed_order {
        let mapped_bytes =
            runtime.allocation_bytes(order.len(), std::mem::size_of::<LayerFace>())?;
        let order_bytes =
            runtime.allocation_bytes(order.capacity(), std::mem::size_of::<usize>())?;
        runtime.add_verification_storage(mapped_bytes)?;
        runtime.ensure_transient_exact_storage(
            order_bytes
                .checked_add(mapped_bytes)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        let mut mapped = Vec::new();
        mapped.try_reserve_exact(order.len()).map_err(|_| {
            runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
        })?;
        for index in order {
            mapped.push(embedding.faces[index].source.layer);
        }
        Some(mapped)
    } else {
        None
    };
    if layer_order.global_bottom_to_top != recomputed_global {
        return Err(certificate_failure());
    }
    let maximum_ply = cells
        .iter()
        .map(|cell| cell.covering_faces.len())
        .max()
        .unwrap_or(1);
    let Some(summary) = layer_order.proof_summary else {
        return Err(certificate_failure());
    };
    if summary.material_faces != embedding.faces.len()
        || summary.overlap_face_pairs != pair_values.len()
        || summary.overlap_cells != cells.len()
        || summary.constraints != runtime.work.constraints
        || summary.search_nodes != runtime.work.search_nodes
        || summary.maximum_ply != maximum_ply
    {
        return Err(certificate_failure());
    }
    let serialized_bytes = serialized_certificate_size(layer_order, runtime)?;
    if summary.certificate_bytes != runtime.work.certificate_bytes
        || summary.certificate_bytes != serialized_bytes
    {
        return Err(certificate_failure());
    }
    drop(recomputed_global);
    drop(expected_face_pair_orders);
    drop(internal_cells);
    drop(snapshot_cell_keys);
    drop(internal_cell_keys);
    drop(expected_material_faces);
    runtime.clear_verification_storage();
    runtime.checkpoint(None)?;
    Ok(())
}

#[derive(Clone, PartialEq, Eq)]
struct ConstraintProblem {
    variables: Vec<(usize, usize)>,
    constraints: Vec<TupleConstraint>,
    fixed_assignments: Vec<Option<bool>>,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct PairValues(Vec<((usize, usize), bool)>);

impl PairValues {
    fn try_from_parallel(
        variables: &[(usize, usize)],
        assignment: &[bool],
    ) -> Result<Self, std::collections::TryReserveError> {
        let mut values = Vec::new();
        values.try_reserve_exact(variables.len())?;
        values.extend(variables.iter().copied().zip(assignment.iter().copied()));
        Ok(Self(values))
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn get(&self, key: &(usize, usize)) -> Option<&bool> {
        self.0
            .binary_search_by_key(key, |(pair, _)| *pair)
            .ok()
            .map(|index| &self.0[index].1)
    }

    fn contains_key(&self, key: &(usize, usize)) -> bool {
        self.get(key).is_some()
    }

    fn keys(&self) -> impl ExactSizeIterator<Item = &(usize, usize)> {
        self.0.iter().map(|(pair, _)| pair)
    }

    fn iter(&self) -> impl ExactSizeIterator<Item = &((usize, usize), bool)> {
        self.0.iter()
    }

    #[cfg(test)]
    fn insert(&mut self, key: (usize, usize), value: bool) {
        match self.0.binary_search_by_key(&key, |(pair, _)| *pair) {
            Ok(index) => self.0[index].1 = value,
            Err(index) => self.0.insert(index, (key, value)),
        }
    }
}

#[derive(Clone, Copy)]
enum ConstraintStorageScope {
    Primary,
    Verification,
}

fn add_constraint_problem_storage<O: GlobalFlatFoldabilityObserver + ?Sized>(
    runtime: &mut Runtime<'_, O>,
    scope: ConstraintStorageScope,
    additional: usize,
) -> FacewiseResult<()> {
    match scope {
        ConstraintStorageScope::Primary => runtime.add_constraint_storage(additional),
        ConstraintStorageScope::Verification => runtime.add_verification_storage(additional),
    }
}

fn ensure_constraint_scope_transient<O: GlobalFlatFoldabilityObserver + ?Sized>(
    runtime: &Runtime<'_, O>,
    scope: ConstraintStorageScope,
    additional: usize,
) -> FacewiseResult<()> {
    match scope {
        ConstraintStorageScope::Primary => runtime.ensure_constraint_transient_storage(additional),
        ConstraintStorageScope::Verification => runtime.ensure_transient_exact_storage(additional),
    }
}

fn ensure_constraint_construction_headroom<O: GlobalFlatFoldabilityObserver + ?Sized>(
    runtime: &Runtime<'_, O>,
    scope: ConstraintStorageScope,
) -> FacewiseResult<()> {
    let maximum_inner_bytes = 6_usize
        .checked_mul(std::mem::size_of::<usize>())
        .and_then(|total| total.checked_add(64 * std::mem::size_of::<u8>()))
        .and_then(|total| total.checked_add(4 * std::mem::size_of::<usize>()))
        .and_then(|total| total.checked_add(std::mem::size_of::<TupleConstraint>()))
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    ensure_constraint_scope_transient(runtime, scope, maximum_inner_bytes)
}

fn build_constraint_problem<O: GlobalFlatFoldabilityObserver + ?Sized>(
    embedding: &FlatEmbedding,
    pairs: &[OverlapPair],
    cells: &[OverlapCell],
    runtime: &mut Runtime<'_, O>,
    record_work: bool,
) -> FacewiseResult<ConstraintProblem> {
    let storage_scope = if record_work {
        ConstraintStorageScope::Primary
    } else {
        ConstraintStorageScope::Verification
    };
    runtime.checkpoint(None)?;
    let variable_bytes =
        runtime.allocation_bytes(pairs.len(), std::mem::size_of::<(usize, usize)>())?;
    add_constraint_problem_storage(runtime, storage_scope, variable_bytes)?;
    let mut variables = Vec::new();
    variables
        .try_reserve_exact(pairs.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let mut variable_poll = 0_usize;
    for pair in pairs {
        runtime.poll_control(&mut variable_poll)?;
        variables.push(ordered_pair(pair.first, pair.second));
    }
    variables.sort_unstable();
    let original_variable_count = variables.len();
    variables.dedup();
    if variables.len() != original_variable_count {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
            },
        ));
    }
    let mut constraints = Vec::new();
    let fixed_assignment_bytes =
        runtime.allocation_bytes(variables.len(), std::mem::size_of::<Option<bool>>())?;
    add_constraint_problem_storage(runtime, storage_scope, fixed_assignment_bytes)?;
    let mut fixed_assignments = Vec::new();
    fixed_assignments
        .try_reserve_exact(variables.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    fixed_assignments.resize(variables.len(), None);

    for (variable, &(first, second)) in variables.iter().enumerate() {
        runtime.checkpoint(None)?;
        ensure_constraint_construction_headroom(runtime, storage_scope)?;
        push_constraint(
            &mut constraints,
            TupleConstraint {
                kind: FacewiseConstraintKind::Antisymmetry,
                variables: vec![variable],
                allowed_rows: vec![0, 1],
                faces: vec![first, second],
                supporting_cell: supporting_cell(cells, &[first, second], runtime)?,
            },
            runtime,
            storage_scope,
        )?;
    }

    for hinge in &embedding.hinges {
        runtime.checkpoint(None)?;
        let key = ordered_pair(hinge.first_face, hinge.second_face);
        let Ok(variable) = variables.binary_search(&key) else {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                    reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
                },
            ));
        };
        let canonical_value = mountain_valley_canonical_value(
            hinge.assignment,
            embedding.faces[hinge.first_face].front_up,
            hinge.first_face,
            hinge.second_face,
        );
        if fixed_assignments[variable].is_some_and(|existing| existing != canonical_value) {
            ensure_constraint_construction_headroom(runtime, storage_scope)?;
            let constraint = TupleConstraint {
                kind: FacewiseConstraintKind::MountainValley,
                variables: vec![variable],
                allowed_rows: vec![u8::from(canonical_value)],
                faces: vec![key.0, key.1],
                supporting_cell: supporting_cell(cells, &[key.0, key.1], runtime)?,
            };
            return Err(constraint_contradiction(&constraint, embedding));
        }
        fixed_assignments[variable] = Some(canonical_value);
        ensure_constraint_construction_headroom(runtime, storage_scope)?;
        push_constraint(
            &mut constraints,
            TupleConstraint {
                kind: FacewiseConstraintKind::MountainValley,
                variables: vec![variable],
                allowed_rows: vec![u8::from(canonical_value)],
                faces: vec![key.0, key.1],
                supporting_cell: supporting_cell(cells, &[key.0, key.1], runtime)?,
            },
            runtime,
            storage_scope,
        )?;
    }

    for cell in cells {
        runtime.checkpoint(None)?;
        for first_index in 0..cell.covering_faces.len() {
            for second_index in (first_index + 1)..cell.covering_faces.len() {
                for third_index in (second_index + 1)..cell.covering_faces.len() {
                    let first = cell.covering_faces[first_index];
                    let second = cell.covering_faces[second_index];
                    let third = cell.covering_faces[third_index];
                    let relations = [(first, second), (second, third), (third, first)];
                    let faces = [first, second, third];
                    let constraint = relation_constraint(
                        RelationConstraintInput {
                            kind: FacewiseConstraintKind::Transitivity,
                            relations: &relations,
                            faces: &faces,
                            supporting_cell: Some(cell.key),
                            variable_pairs: &variables,
                        },
                        |relations| !(relations[0] == relations[1] && relations[1] == relations[2]),
                        runtime,
                        storage_scope,
                    )?;
                    push_constraint(&mut constraints, constraint, runtime, storage_scope)?;
                }
            }
        }
    }

    for hinge in &embedding.hinges {
        runtime.checkpoint(None)?;
        for face in 0..embedding.faces.len() {
            if face == hinge.first_face || face == hinge.second_face {
                continue;
            }
            if !segment_overlaps_face_interior(
                &hinge.first_point,
                &hinge.second_point,
                &embedding.faces[face].polygon,
                runtime,
            )? {
                continue;
            }
            let mut evidence_faces = vec![hinge.first_face, hinge.second_face, face];
            evidence_faces.sort_unstable();
            let support = supporting_cell(cells, &evidence_faces, runtime)?
                .ok_or_else(certificate_failure)?;
            let relations = [(hinge.first_face, face), (hinge.second_face, face)];
            let constraint = relation_constraint(
                RelationConstraintInput {
                    kind: FacewiseConstraintKind::TacoTortilla,
                    relations: &relations,
                    faces: &evidence_faces,
                    supporting_cell: Some(support),
                    variable_pairs: &variables,
                },
                |relations| relations[0] == relations[1],
                runtime,
                storage_scope,
            )?;
            push_constraint(&mut constraints, constraint, runtime, storage_scope)?;
        }
    }

    for first_hinge_index in 0..embedding.hinges.len() {
        runtime.checkpoint(None)?;
        for second_hinge_index in (first_hinge_index + 1)..embedding.hinges.len() {
            let first_hinge = &embedding.hinges[first_hinge_index];
            let second_hinge = &embedding.hinges[second_hinge_index];
            if !segments_overlap_in_positive_length(
                &first_hinge.first_point,
                &first_hinge.second_point,
                &second_hinge.first_point,
                &second_hinge.second_point,
                runtime,
            )? {
                continue;
            }
            let mut evidence_faces = vec![
                first_hinge.first_face,
                first_hinge.second_face,
                second_hinge.first_face,
                second_hinge.second_face,
            ];
            evidence_faces.sort_unstable();
            evidence_faces.dedup();
            if evidence_faces.len() != 4 {
                return Err(FacewiseAbort::Unknown(
                    GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                        reason:
                            FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
                    },
                ));
            }
            if !all_face_pairs_overlap(&evidence_faces, &variables) {
                continue;
            }
            let support = supporting_cell(cells, &evidence_faces, runtime)?
                .ok_or_else(certificate_failure)?;
            let relations = [
                (first_hinge.first_face, first_hinge.second_face),
                (second_hinge.first_face, second_hinge.second_face),
                (second_hinge.first_face, first_hinge.second_face),
                (first_hinge.first_face, second_hinge.second_face),
                (first_hinge.first_face, second_hinge.first_face),
                (first_hinge.second_face, second_hinge.second_face),
            ];
            let constraint = relation_constraint(
                RelationConstraintInput {
                    kind: FacewiseConstraintKind::TacoTaco,
                    relations: &relations,
                    faces: &evidence_faces,
                    supporting_cell: Some(support),
                    variable_pairs: &variables,
                },
                taco_taco_source_tuple_accepts,
                runtime,
                storage_scope,
            )?;
            push_constraint(&mut constraints, constraint, runtime, storage_scope)?;
        }
    }

    // The current target class has only M/V hinges. `Auxiliary` edges are
    // topology annotations (`AuxiliaryIgnored`), not unfolded material
    // creases, so tortilla-tortilla constraints are intentionally zero.
    constraints.sort_unstable_by(compare_constraints);
    if record_work {
        runtime.set_constraints(constraints.len())?;
    } else if constraints.len() > runtime.limits.max_constraints {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ConstraintLimitReached {
                limit: runtime.limits.max_constraints,
                observed: constraints.len(),
            },
        ));
    }
    Ok(ConstraintProblem {
        variables,
        constraints,
        fixed_assignments,
    })
}

fn taco_taco_source_tuple_accepts(relations: &[bool]) -> bool {
    relations.len() == 6
        && TACO_TACO_VALID_SOURCE_TUPLES.iter().any(|tuple| {
            tuple
                .as_bytes()
                .iter()
                .zip(relations)
                .all(|(symbol, relation)| (*symbol == b'1') == *relation)
        })
}

fn mountain_valley_canonical_value(
    assignment: FoldAssignment,
    first_front_up: bool,
    first_face: usize,
    second_face: usize,
) -> bool {
    let second_above_first = (assignment == FoldAssignment::Mountain) == first_front_up;
    if first_face < second_face {
        second_above_first
    } else {
        !second_above_first
    }
}

fn push_constraint<O: GlobalFlatFoldabilityObserver + ?Sized>(
    constraints: &mut Vec<TupleConstraint>,
    constraint: TupleConstraint,
    runtime: &mut Runtime<'_, O>,
    storage_scope: ConstraintStorageScope,
) -> FacewiseResult<()> {
    runtime.checkpoint(None)?;
    let observed = match constraints.len().checked_add(1) {
        Some(observed) => observed,
        None => {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ConstraintLimitReached {
                    limit: runtime.limits.max_constraints,
                    observed: usize::MAX,
                },
            ));
        }
    };
    if observed > runtime.limits.max_constraints {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ConstraintLimitReached {
                limit: runtime.limits.max_constraints,
                observed,
            },
        ));
    }
    let nested_bytes = runtime
        .allocation_bytes(
            constraint.variables.capacity(),
            std::mem::size_of::<usize>(),
        )?
        .checked_add(runtime.allocation_bytes(
            constraint.allowed_rows.capacity(),
            std::mem::size_of::<u8>(),
        )?)
        .and_then(|total| {
            total.checked_add(
                runtime
                    .allocation_bytes(constraint.faces.capacity(), std::mem::size_of::<usize>())
                    .ok()?,
            )
        })
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    let prior_capacity = constraints.capacity();
    let next_capacity = if constraints.len() == prior_capacity {
        next_vector_capacity(
            prior_capacity,
            constraints.len(),
            runtime.limits.max_constraints,
            runtime,
        )?
    } else {
        prior_capacity
    };
    let outer_bytes = runtime.allocation_bytes(
        next_capacity - prior_capacity,
        std::mem::size_of::<TupleConstraint>(),
    )?;
    let additional = outer_bytes
        .checked_add(nested_bytes)
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    let old_outer_bytes = if next_capacity > prior_capacity {
        runtime.allocation_bytes(prior_capacity, std::mem::size_of::<TupleConstraint>())?
    } else {
        0
    };
    let peak_additional = additional
        .checked_add(old_outer_bytes)
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    ensure_constraint_scope_transient(runtime, storage_scope, peak_additional)?;
    add_constraint_problem_storage(runtime, storage_scope, additional)?;
    if next_capacity > prior_capacity {
        // A grow may allocate and move the new buffer before releasing the
        // old allocation. `peak_additional` admitted both buffers before the
        // retained accounting was committed.
        constraints
            .try_reserve_exact(next_capacity - constraints.len())
            .map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
    }
    constraints.push(constraint);
    Ok(())
}

struct RelationConstraintInput<'a> {
    kind: FacewiseConstraintKind,
    relations: &'a [(usize, usize)],
    faces: &'a [usize],
    supporting_cell: Option<OverlapCellKey>,
    variable_pairs: &'a [(usize, usize)],
}

fn relation_constraint<O, F>(
    input: RelationConstraintInput<'_>,
    accepts: F,
    runtime: &Runtime<'_, O>,
    storage_scope: ConstraintStorageScope,
) -> FacewiseResult<TupleConstraint>
where
    O: GlobalFlatFoldabilityObserver + ?Sized,
    F: Fn(&[bool]) -> bool,
{
    let RelationConstraintInput {
        kind,
        relations,
        faces,
        supporting_cell,
        variable_pairs,
    } = input;
    ensure_constraint_construction_headroom(runtime, storage_scope)?;
    let mut variables = Vec::new();
    variables
        .try_reserve_exact(relations.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for &(first, second) in relations {
        if first == second {
            return Err(FacewiseAbort::Execution(internal_error()));
        }
        let Ok(variable) = variable_pairs.binary_search(&ordered_pair(first, second)) else {
            return Err(certificate_failure());
        };
        variables.push(variable);
    }
    variables.sort_unstable();
    variables.dedup();
    if variables.len() > 6 {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    let row_count = 1_u16
        .checked_shl(u32::try_from(variables.len()).map_err(|_| internal_abort())?)
        .ok_or_else(internal_abort)?;
    let mut allowed_rows = Vec::new();
    allowed_rows
        .try_reserve_exact(usize::from(row_count))
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for row in 0..row_count {
        let row = u8::try_from(row).map_err(|_| internal_abort())?;
        let mut relation_values = [false; 6];
        for (index, &(first, second)) in relations.iter().enumerate() {
            relation_values[index] =
                directed_face_above_from_row(first, second, row, &variables, variable_pairs)?;
        }
        if accepts(&relation_values[..relations.len()]) {
            allowed_rows.push(row);
        }
    }
    if allowed_rows.is_empty() {
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    Ok(TupleConstraint {
        kind,
        variables,
        allowed_rows,
        faces: {
            let mut stored_faces = Vec::new();
            stored_faces.try_reserve_exact(faces.len()).map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
            stored_faces.extend_from_slice(faces);
            stored_faces
        },
        supporting_cell,
    })
}

fn directed_face_above_from_row(
    first: usize,
    second: usize,
    row: u8,
    variables: &[usize],
    variable_pairs: &[(usize, usize)],
) -> FacewiseResult<bool> {
    let pair = ordered_pair(first, second);
    let variable = variable_pairs
        .binary_search(&pair)
        .map_err(|_| certificate_failure())?;
    let position = variables
        .binary_search(&variable)
        .map_err(|_| certificate_failure())?;
    let canonical_second_above_first = row & (1_u8 << position) != 0;
    Ok(if first == pair.0 {
        !canonical_second_above_first
    } else {
        canonical_second_above_first
    })
}

fn supporting_cell<O: GlobalFlatFoldabilityObserver + ?Sized>(
    cells: &[OverlapCell],
    faces: &[usize],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Option<OverlapCellKey>> {
    let mut minimum = None;
    let mut control_poll = 0_usize;
    for cell in cells {
        runtime.poll_control(&mut control_poll)?;
        let mut supports_all = true;
        for face in faces {
            runtime.poll_control(&mut control_poll)?;
            if !cell.covering_faces.contains(face) {
                supports_all = false;
                break;
            }
        }
        if supports_all && minimum.is_none_or(|key: OverlapCellKey| cell.key.0 < key.0) {
            minimum = Some(cell.key);
        }
    }
    Ok(minimum)
}

fn all_face_pairs_overlap(faces: &[usize], variable_pairs: &[(usize, usize)]) -> bool {
    (0..faces.len()).all(|first| {
        ((first + 1)..faces.len()).all(|second| {
            variable_pairs
                .binary_search(&ordered_pair(faces[first], faces[second]))
                .is_ok()
        })
    })
}

fn compare_constraints(left: &TupleConstraint, right: &TupleConstraint) -> Ordering {
    constraint_kind_rank(left.kind)
        .cmp(&constraint_kind_rank(right.kind))
        .then_with(|| left.faces.cmp(&right.faces))
        .then_with(|| {
            left.supporting_cell
                .map(|cell| cell.0)
                .cmp(&right.supporting_cell.map(|cell| cell.0))
        })
        .then_with(|| left.variables.cmp(&right.variables))
        .then_with(|| left.allowed_rows.cmp(&right.allowed_rows))
}

const fn constraint_kind_rank(kind: FacewiseConstraintKind) -> u8 {
    match kind {
        FacewiseConstraintKind::Antisymmetry => 0,
        FacewiseConstraintKind::Transitivity => 1,
        FacewiseConstraintKind::TortillaTortilla => 2,
        FacewiseConstraintKind::TacoTortilla => 3,
        FacewiseConstraintKind::TacoTaco => 4,
        FacewiseConstraintKind::MountainValley => 5,
    }
}

fn segment_overlaps_face_interior<O: GlobalFlatFoldabilityObserver + ?Sized>(
    first: &Point,
    second: &Point,
    polygon: &[Point],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    if first == second {
        return Ok(false);
    }
    let mut lower = Rational::zero();
    let mut upper = Rational::from_integer(1.into());
    for edge_index in 0..polygon.len() {
        let edge_first = &polygon[edge_index];
        let edge_second = &polygon[(edge_index + 1) % polygon.len()];
        let first_side = cross(edge_first, edge_second, first, runtime)?;
        let second_side = cross(edge_first, edge_second, second, runtime)?;
        if first_side.is_zero() && second_side.is_zero() {
            return Ok(false);
        }
        if !first_side.is_positive() && !second_side.is_positive() {
            return Ok(false);
        }
        if first_side.is_negative() && !second_side.is_negative() {
            let crossing = div(
                &first_side,
                &sub(&first_side, &second_side, runtime)?,
                runtime,
            )?;
            if cmp(&crossing, &lower, runtime)? == Ordering::Greater {
                lower = crossing;
            }
        } else if !first_side.is_negative() && second_side.is_negative() {
            let crossing = div(
                &first_side,
                &sub(&first_side, &second_side, runtime)?,
                runtime,
            )?;
            if cmp(&crossing, &upper, runtime)? == Ordering::Less {
                upper = crossing;
            }
        }
        if cmp(&lower, &upper, runtime)? != Ordering::Less {
            return Ok(false);
        }
    }
    let clipped_first = interpolate(first, second, &lower, runtime)?;
    let clipped_second = interpolate(first, second, &upper, runtime)?;
    let representative = midpoint(&clipped_first, &clipped_second, runtime)?;
    point_in_convex_polygon(&representative, polygon, runtime)
}

fn segments_overlap_in_positive_length<O: GlobalFlatFoldabilityObserver + ?Sized>(
    first_start: &Point,
    first_end: &Point,
    second_start: &Point,
    second_end: &Point,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<bool> {
    if first_start == first_end || second_start == second_end {
        return Ok(false);
    }
    if !cross(first_start, first_end, second_start, runtime)?.is_zero()
        || !cross(first_start, first_end, second_end, runtime)?.is_zero()
    {
        return Ok(false);
    }
    let (first_min, first_max, second_min, second_max) = if first_start.x != first_end.x {
        let (first_min, first_max) = ordered_rationals(&first_start.x, &first_end.x);
        let (second_min, second_max) = ordered_rationals(&second_start.x, &second_end.x);
        (first_min, first_max, second_min, second_max)
    } else {
        let (first_min, first_max) = ordered_rationals(&first_start.y, &first_end.y);
        let (second_min, second_max) = ordered_rationals(&second_start.y, &second_end.y);
        (first_min, first_max, second_min, second_max)
    };
    let lower = if cmp(first_min, second_min, runtime)? == Ordering::Greater {
        first_min
    } else {
        second_min
    };
    let upper = if cmp(first_max, second_max, runtime)? == Ordering::Less {
        first_max
    } else {
        second_max
    };
    Ok(cmp(lower, upper, runtime)? == Ordering::Less)
}

fn ordered_rationals<'a>(
    first: &'a Rational,
    second: &'a Rational,
) -> (&'a Rational, &'a Rational) {
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}

fn constraint_contradiction(
    constraint: &TupleConstraint,
    embedding: &FlatEmbedding,
) -> FacewiseAbort {
    let faces = constraint
        .faces
        .iter()
        .filter_map(|index| embedding.faces.get(*index))
        .map(|face| face.source.layer)
        .collect::<Vec<_>>();
    if faces.len() != constraint.faces.len() {
        return FacewiseAbort::Execution(internal_error());
    }
    FacewiseAbort::Impossible(
        GlobalFlatFoldabilityImpossibleReason::FacewiseConstraintContradiction {
            constraint_kind: constraint.kind,
            faces,
            supporting_cell: constraint.supporting_cell,
        },
    )
}

fn certificate_failure() -> FacewiseAbort {
    FacewiseAbort::Unknown(GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
        reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
    })
}

fn verify_facewise_certificate<O: GlobalFlatFoldabilityObserver + ?Sized>(
    embedding: &FlatEmbedding,
    pairs: &[OverlapPair],
    cells: &[OverlapCell],
    problem: &ConstraintProblem,
    assignment: &[bool],
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    runtime.clear_verification_storage();
    verify_embedding_certificate(embedding, runtime)?;
    // Every verifier workspace value is local to
    // `verify_embedding_certificate` and has been dropped on return.
    runtime.clear_verification_storage();
    let verification_base = runtime.verification_storage_bytes();
    let regenerated = match build_constraint_problem(embedding, pairs, cells, runtime, false) {
        Ok(regenerated) => regenerated,
        Err(abort) => {
            runtime.restore_verification_storage(verification_base);
            return Err(abort);
        }
    };
    if regenerated != *problem || assignment.len() != problem.variables.len() {
        drop(regenerated);
        runtime.restore_verification_storage(verification_base);
        return Err(certificate_failure());
    }
    if problem
        .fixed_assignments
        .iter()
        .zip(assignment)
        .any(|(fixed, value)| fixed.is_some_and(|fixed| fixed != *value))
    {
        drop(regenerated);
        runtime.restore_verification_storage(verification_base);
        return Err(certificate_failure());
    }
    for constraint in &regenerated.constraints {
        if let Err(abort) = runtime.checkpoint(None) {
            drop(regenerated);
            runtime.restore_verification_storage(verification_base);
            return Err(abort);
        }
        if !fresh_constraint_accepts(constraint, assignment) {
            drop(regenerated);
            runtime.restore_verification_storage(verification_base);
            return Err(certificate_failure());
        }
    }
    drop(regenerated);
    runtime.restore_verification_storage(verification_base);
    runtime.add_verification_storage(runtime.allocation_bytes(
        problem.variables.len(),
        std::mem::size_of::<((usize, usize), bool)>(),
    )?)?;
    let pair_values = PairValues::try_from_parallel(&problem.variables, assignment)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    if pair_values.len() != pairs.len() {
        return Err(certificate_failure());
    }
    verify_geometric_constraints_direct(embedding, cells, problem, &pair_values, runtime)?;

    runtime.add_verification_storage(runtime.allocation_bytes(
        pair_values.len(),
        std::mem::size_of::<((usize, usize), Rational)>(),
    )?)?;
    let mut actual_pair_areas = Vec::new();
    actual_pair_areas
        .try_reserve_exact(pair_values.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for first in 0..embedding.faces.len() {
        runtime.checkpoint(None)?;
        for second in (first + 1)..embedding.faces.len() {
            let intersection = convex_polygon_intersection(
                &embedding.faces[first].polygon,
                &embedding.faces[second].polygon,
                runtime,
            )?;
            if intersection.len() >= 3 && signed_double_area(&intersection, runtime)?.is_positive()
            {
                let area = signed_double_area(&intersection, runtime)?;
                let area_storage = exact::rational_storage_bytes(&area)?;
                runtime.add_verification_storage(area_storage)?;
                actual_pair_areas.push(((first, second), area));
            }
        }
    }
    if pair_values
        .keys()
        .copied()
        .ne(actual_pair_areas.iter().map(|(pair, _)| *pair))
    {
        return Err(certificate_failure());
    }
    for first in 0..embedding.faces.len() {
        runtime.checkpoint(None)?;
        for second in (first + 1)..embedding.faces.len() {
            for third in (second + 1)..embedding.faces.len() {
                let first_second = convex_polygon_intersection(
                    &embedding.faces[first].polygon,
                    &embedding.faces[second].polygon,
                    runtime,
                )?;
                if first_second.len() < 3
                    || !signed_double_area(&first_second, runtime)?.is_positive()
                {
                    continue;
                }
                let triple_scope_base = runtime.verification_storage_bytes();
                let first_second_bytes =
                    exact_storage_bytes_points(&first_second)?
                        .checked_add(runtime.allocation_bytes(
                            first_second.capacity(),
                            std::mem::size_of::<Point>(),
                        )?)
                        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
                runtime.add_verification_storage(first_second_bytes)?;
                let common = convex_polygon_intersection(
                    &first_second,
                    &embedding.faces[third].polygon,
                    runtime,
                )?;
                let common_is_positive =
                    common.len() >= 3 && signed_double_area(&common, runtime)?.is_positive();
                drop(common);
                drop(first_second);
                runtime.restore_verification_storage(triple_scope_base);
                if !common_is_positive {
                    continue;
                }
                if supporting_cell(cells, &[first, second, third], runtime)?.is_none() {
                    return Err(certificate_failure());
                }
                let first_second_order = face_above(first, second, &pair_values)?;
                let second_third_order = face_above(second, third, &pair_values)?;
                let third_first_order = face_above(third, first, &pair_values)?;
                if first_second_order == second_third_order
                    && second_third_order == third_first_order
                {
                    return Err(certificate_failure());
                }
            }
        }
    }

    let cell_key_entry_bytes = std::mem::size_of::<[u8; 32]>()
        .checked_add(3 * std::mem::size_of::<usize>())
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    runtime
        .add_verification_storage(runtime.allocation_bytes(cells.len(), cell_key_entry_bytes)?)?;
    let mut cell_keys = HashSet::new();
    cell_keys
        .try_reserve(cells.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    runtime.add_verification_storage(
        runtime.allocation_bytes(cells.len(), std::mem::size_of::<Rational>())?,
    )?;
    let mut verified_cell_areas = Vec::new();
    verified_cell_areas
        .try_reserve_exact(cells.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for cell in cells {
        runtime.checkpoint(None)?;
        let cell_scope_base = runtime.verification_storage_bytes();
        let covering_set_bytes = runtime.allocation_bytes(
            cell.covering_faces.len(),
            std::mem::size_of::<usize>()
                .checked_add(3 * std::mem::size_of::<usize>())
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        let boundary_set_structure = runtime.allocation_bytes(
            cell.boundary.len(),
            std::mem::size_of::<Point>()
                .checked_add(3 * std::mem::size_of::<usize>())
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        let boundary_set_exact = exact_storage_bytes_points(&cell.boundary)?;
        let expected_face_bytes =
            runtime.allocation_bytes(embedding.faces.len(), std::mem::size_of::<usize>())?;
        let ordered_face_bytes =
            runtime.allocation_bytes(cell.covering_faces.len(), std::mem::size_of::<usize>())?;
        let cell_temporary_bytes = covering_set_bytes
            .checked_add(boundary_set_structure)
            .and_then(|total| total.checked_add(boundary_set_exact))
            .and_then(|total| total.checked_add(expected_face_bytes))
            .and_then(|total| total.checked_add(ordered_face_bytes))
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
        runtime.add_verification_storage(cell_temporary_bytes)?;
        let covering_faces_are_unique = cell
            .covering_faces
            .windows(2)
            .all(|faces| faces[0] < faces[1]);
        let mut boundary_points_are_unique = true;
        let mut uniqueness_poll = 0_usize;
        for first in 0..cell.boundary.len() {
            for second in (first + 1)..cell.boundary.len() {
                runtime.poll_control(&mut uniqueness_poll)?;
                if cell.boundary[first] == cell.boundary[second] {
                    boundary_points_are_unique = false;
                    break;
                }
            }
            if !boundary_points_are_unique {
                break;
            }
        }
        let mut strictly_convex = cell.boundary.len() >= 3;
        for index in 0..cell.boundary.len() {
            strictly_convex &= cross(
                &cell.boundary[index],
                &cell.boundary[(index + 1) % cell.boundary.len()],
                &cell.boundary[(index + 2) % cell.boundary.len()],
                runtime,
            )?
            .is_positive();
        }
        let cell_area = signed_double_area(&cell.boundary, runtime)?;
        if cell.boundary.len() < 3
            || !cell_area.is_positive()
            || overlap_cell_key(
                &cell.boundary,
                &cell.covering_faces,
                &embedding.faces,
                runtime,
            )? != cell.key
            || !cell_keys.insert(cell.key.0)
            || !boundary_points_are_unique
            || !strictly_convex
            || !covering_faces_are_unique
        {
            return Err(certificate_failure());
        }
        let mut expected_faces = Vec::new();
        expected_faces
            .try_reserve_exact(embedding.faces.len())
            .map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
        let representative = representative_point(&cell.boundary, runtime)?;
        for (face_index, face) in embedding.faces.iter().enumerate() {
            if point_in_convex_polygon(&representative, &face.polygon, runtime)? {
                expected_faces.push(face_index);
            }
        }
        drop(representative);
        if expected_faces != cell.covering_faces {
            return Err(certificate_failure());
        }
        for (face_index, face) in embedding.faces.iter().enumerate() {
            let intersection = convex_polygon_intersection(&cell.boundary, &face.polygon, runtime)?;
            let intersection_area = if intersection.len() >= 3 {
                signed_double_area(&intersection, runtime)?
            } else {
                Rational::zero()
            };
            if cell.covering_faces.contains(&face_index) {
                if intersection_area != cell_area {
                    return Err(certificate_failure());
                }
            } else if intersection_area.is_positive() {
                return Err(certificate_failure());
            }
        }
        let ordered_faces = order_cell_faces(&cell.covering_faces, &pair_values, runtime)?;
        if ordered_faces.len() != cell.covering_faces.len() {
            return Err(certificate_failure());
        }
        let mut order_verification_poll = 0_usize;
        for lower_index in 0..ordered_faces.len() {
            for upper_index in (lower_index + 1)..ordered_faces.len() {
                runtime.poll_control(&mut order_verification_poll)?;
                if !face_is_below(
                    ordered_faces[lower_index],
                    ordered_faces[upper_index],
                    &pair_values,
                )? {
                    return Err(certificate_failure());
                }
            }
        }
        let retained_cell_area_bytes = exact::rational_storage_bytes(&cell_area)?;
        drop(ordered_faces);
        drop(expected_faces);
        runtime.restore_verification_storage(cell_scope_base);
        runtime.add_verification_storage(retained_cell_area_bytes)?;
        verified_cell_areas.push(cell_area);
    }
    for first_cell in 0..cells.len() {
        runtime.checkpoint(None)?;
        for second_cell in (first_cell + 1)..cells.len() {
            let intersection = convex_polygon_intersection(
                &cells[first_cell].boundary,
                &cells[second_cell].boundary,
                runtime,
            )?;
            if intersection.len() >= 3 && signed_double_area(&intersection, runtime)?.is_positive()
            {
                return Err(certificate_failure());
            }
        }
    }
    for ((first, second), expected_area) in &actual_pair_areas {
        runtime.checkpoint(None)?;
        let mut covered_area = Rational::zero();
        for (cell, area) in cells.iter().zip(&verified_cell_areas) {
            if cell.covering_faces.contains(first) && cell.covering_faces.contains(second) {
                covered_area = add(&covered_area, area, runtime)?;
                runtime.ensure_transient_exact_storage(exact::rational_storage_bytes(
                    &covered_area,
                )?)?;
            }
        }
        if &covered_area != expected_area {
            return Err(certificate_failure());
        }
    }
    for (face_index, face) in embedding.faces.iter().enumerate() {
        runtime.checkpoint(None)?;
        let expected_area = signed_double_area(&face.polygon, runtime)?;
        let mut covered_area = Rational::zero();
        for (cell, area) in cells.iter().zip(&verified_cell_areas) {
            if cell.covering_faces.contains(&face_index) {
                covered_area = add(&covered_area, area, runtime)?;
                runtime.ensure_transient_exact_storage(exact::rational_storage_bytes(
                    &covered_area,
                )?)?;
            }
        }
        if covered_area != expected_area {
            return Err(certificate_failure());
        }
    }
    for &((first, second), _) in &actual_pair_areas {
        if supporting_cell(cells, &[first, second], runtime)?.is_none() {
            return Err(certificate_failure());
        }
    }
    drop(verified_cell_areas);
    drop(cell_keys);
    drop(actual_pair_areas);
    drop(pair_values);
    runtime.clear_verification_storage();
    runtime.checkpoint(None)?;
    Ok(())
}

fn verify_embedding_certificate<O: GlobalFlatFoldabilityObserver + ?Sized>(
    embedding: &FlatEmbedding,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    if embedding.faces.is_empty()
        || embedding.reference_face != 0
        || embedding.faces[0].transform != Transform::identity()
        || !embedding.faces[0].front_up
        || embedding.material_internal_edge_count != embedding.hinges.len()
    {
        return Err(certificate_failure());
    }
    let face_set_entry_bytes = std::mem::size_of::<ori_domain::FaceId>()
        .checked_add(std::mem::size_of::<ori_topology::FaceKey>())
        .and_then(|total| total.checked_add(4 * std::mem::size_of::<usize>()))
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    runtime.add_verification_storage(
        runtime.allocation_bytes(embedding.faces.len(), face_set_entry_bytes)?,
    )?;
    let mut face_ids = HashSet::new();
    face_ids
        .try_reserve(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let mut face_keys = HashSet::new();
    face_keys
        .try_reserve(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for (face_index, face) in embedding.faces.iter().enumerate() {
        runtime.checkpoint(None)?;
        if !face_ids.insert(face.source.layer.face_id)
            || !face_keys.insert(face.source.layer.face_key)
            || (face_index > 0
                && embedding.faces[face_index - 1].source.layer.face_key
                    >= face.source.layer.face_key)
        {
            return Err(certificate_failure());
        }
        let determinant = sub(
            &mul(&face.transform.m00, &face.transform.m11, runtime)?,
            &mul(&face.transform.m01, &face.transform.m10, runtime)?,
            runtime,
        )?;
        if determinant.is_zero() || determinant.is_positive() != face.front_up {
            return Err(certificate_failure());
        }
        let recomputed_structure_bytes = runtime.allocation_bytes(
            face.source.source_polygon.len(),
            std::mem::size_of::<Point>(),
        )?;
        runtime.ensure_transient_exact_storage(recomputed_structure_bytes)?;
        let mut recomputed_polygon = Vec::new();
        recomputed_polygon
            .try_reserve_exact(face.source.source_polygon.len())
            .map_err(|_| {
                runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
            })?;
        let mut recomputed_exact_bytes = 0_usize;
        let mut recompute_poll = 0_usize;
        for source_point in &face.source.source_polygon {
            runtime.poll_control(&mut recompute_poll)?;
            let recomputed_point = apply(&face.transform, source_point, runtime)?;
            recomputed_exact_bytes = recomputed_exact_bytes
                .checked_add(exact_storage_bytes_point(&recomputed_point)?)
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
            runtime.ensure_transient_exact_storage(
                recomputed_structure_bytes
                    .checked_add(recomputed_exact_bytes)
                    .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
            )?;
            recomputed_polygon.push(recomputed_point);
        }
        let area = signed_double_area(&recomputed_polygon, runtime)?;
        runtime.ensure_transient_exact_storage(
            recomputed_structure_bytes
                .checked_add(recomputed_exact_bytes)
                .and_then(|total| total.checked_add(exact::rational_storage_bytes(&area).ok()?))
                .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
        )?;
        if area.is_negative() {
            recomputed_polygon.reverse();
        }
        if area.is_zero() || recomputed_polygon != face.polygon {
            return Err(certificate_failure());
        }
    }
    let hinge_entry_bytes = std::mem::size_of::<EdgeId>()
        .checked_add(2 * std::mem::size_of::<usize>())
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    let connectivity_bytes = runtime
        .allocation_bytes(embedding.hinges.len(), hinge_entry_bytes)?
        .checked_add(runtime.allocation_bytes(embedding.faces.len(), std::mem::size_of::<usize>())?)
        .and_then(|total| {
            total.checked_add(
                runtime
                    .allocation_bytes(embedding.faces.len(), std::mem::size_of::<u8>())
                    .ok()?,
            )
        })
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    runtime.add_verification_storage(connectivity_bytes)?;
    let mut hinge_edges = HashSet::new();
    hinge_edges
        .try_reserve(embedding.hinges.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    let mut parents = Vec::new();
    parents
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    parents.extend(0..embedding.faces.len());
    let mut ranks = Vec::new();
    ranks
        .try_reserve_exact(embedding.faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    ranks.resize(embedding.faces.len(), 0_u8);
    for hinge in &embedding.hinges {
        runtime.checkpoint(None)?;
        if hinge.first_face >= embedding.faces.len()
            || hinge.second_face >= embedding.faces.len()
            || hinge.first_face == hinge.second_face
            || hinge.first_point == hinge.second_point
            || !hinge_edges.insert(hinge.edge)
            || !polygon_has_edge(
                &embedding.faces[hinge.first_face].polygon,
                &hinge.first_point,
                &hinge.second_point,
            )
            || !polygon_has_edge(
                &embedding.faces[hinge.second_face].polygon,
                &hinge.first_point,
                &hinge.second_point,
            )
        {
            return Err(certificate_failure());
        }
        let reflection = reflection_across(&hinge.first_point, &hinge.second_point, runtime)?;
        let expected_second = compose(
            &reflection,
            &embedding.faces[hinge.first_face].transform,
            runtime,
        )?;
        if expected_second != embedding.faces[hinge.second_face].transform {
            return Err(certificate_failure());
        }
        union_face_components(
            &mut parents,
            &mut ranks,
            hinge.first_face,
            hinge.second_face,
        );
    }
    let reference_root = find_face_component_root(&mut parents, embedding.reference_face);
    let mut connectivity_poll = 0_usize;
    for face in 0..embedding.faces.len() {
        runtime.poll_control(&mut connectivity_poll)?;
        if find_face_component_root(&mut parents, face) != reference_root {
            return Err(certificate_failure());
        }
    }
    Ok(())
}

fn find_face_component_root(parents: &mut [usize], face: usize) -> usize {
    let mut root = face;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = face;
    while parents[current] != current {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

fn union_face_components(parents: &mut [usize], ranks: &mut [u8], first: usize, second: usize) {
    let first_root = find_face_component_root(parents, first);
    let second_root = find_face_component_root(parents, second);
    if first_root == second_root {
        return;
    }
    match ranks[first_root].cmp(&ranks[second_root]) {
        Ordering::Less => parents[first_root] = second_root,
        Ordering::Greater => parents[second_root] = first_root,
        Ordering::Equal => {
            parents[second_root] = first_root;
            ranks[first_root] = ranks[first_root].saturating_add(1);
        }
    }
}

fn polygon_has_edge(polygon: &[Point], first: &Point, second: &Point) -> bool {
    (0..polygon.len()).any(|index| {
        let current = &polygon[index];
        let next = &polygon[(index + 1) % polygon.len()];
        (current == first && next == second) || (current == second && next == first)
    })
}

fn verify_geometric_constraints_direct<O: GlobalFlatFoldabilityObserver + ?Sized>(
    embedding: &FlatEmbedding,
    cells: &[OverlapCell],
    problem: &ConstraintProblem,
    pair_values: &PairValues,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    if embedding.material_internal_edge_count != embedding.hinges.len() {
        return Err(certificate_failure());
    }
    let mut expected_constraint_count = pair_values.len();

    for hinge in &embedding.hinges {
        runtime.checkpoint(None)?;
        expected_constraint_count = checked_certificate_count(expected_constraint_count, 1)?;
        let expected_canonical = mountain_valley_canonical_value(
            hinge.assignment,
            embedding.faces[hinge.first_face].front_up,
            hinge.first_face,
            hinge.second_face,
        );
        if pair_values
            .get(&ordered_pair(hinge.first_face, hinge.second_face))
            .copied()
            != Some(expected_canonical)
        {
            return Err(certificate_failure());
        }
    }

    let mut transitivity_poll = 0_usize;
    for cell in cells {
        runtime.checkpoint(None)?;
        for first_index in 0..cell.covering_faces.len() {
            for second_index in (first_index + 1)..cell.covering_faces.len() {
                for third_index in (second_index + 1)..cell.covering_faces.len() {
                    runtime.poll_control(&mut transitivity_poll)?;
                    expected_constraint_count =
                        checked_certificate_count(expected_constraint_count, 1)?;
                    let first = cell.covering_faces[first_index];
                    let second = cell.covering_faces[second_index];
                    let third = cell.covering_faces[third_index];
                    let first_second = face_above(first, second, pair_values)?;
                    let second_third = face_above(second, third, pair_values)?;
                    let third_first = face_above(third, first, pair_values)?;
                    if first_second == second_third && second_third == third_first {
                        return Err(certificate_failure());
                    }
                }
            }
        }
    }

    for hinge in &embedding.hinges {
        runtime.checkpoint(None)?;
        for face in 0..embedding.faces.len() {
            if face == hinge.first_face || face == hinge.second_face {
                continue;
            }
            if !segment_overlaps_face_interior(
                &hinge.first_point,
                &hinge.second_point,
                &embedding.faces[face].polygon,
                runtime,
            )? {
                continue;
            }
            let mut evidence_faces = vec![hinge.first_face, hinge.second_face, face];
            evidence_faces.sort_unstable();
            if supporting_cell(cells, &evidence_faces, runtime)?.is_none()
                || face_above(hinge.first_face, face, pair_values)?
                    != face_above(hinge.second_face, face, pair_values)?
            {
                return Err(certificate_failure());
            }
            expected_constraint_count = checked_certificate_count(expected_constraint_count, 1)?;
        }
    }

    for first_hinge_index in 0..embedding.hinges.len() {
        runtime.checkpoint(None)?;
        for second_hinge_index in (first_hinge_index + 1)..embedding.hinges.len() {
            let first_hinge = &embedding.hinges[first_hinge_index];
            let second_hinge = &embedding.hinges[second_hinge_index];
            if !segments_overlap_in_positive_length(
                &first_hinge.first_point,
                &first_hinge.second_point,
                &second_hinge.first_point,
                &second_hinge.second_point,
                runtime,
            )? {
                continue;
            }
            let mut evidence_faces = vec![
                first_hinge.first_face,
                first_hinge.second_face,
                second_hinge.first_face,
                second_hinge.second_face,
            ];
            evidence_faces.sort_unstable();
            evidence_faces.dedup();
            if evidence_faces.len() != 4 {
                return Err(certificate_failure());
            }
            if !all_face_pairs_assigned(&evidence_faces, pair_values) {
                continue;
            }
            if supporting_cell(cells, &evidence_faces, runtime)?.is_none() {
                return Err(certificate_failure());
            }
            let relations = [
                face_above(first_hinge.first_face, first_hinge.second_face, pair_values)?,
                face_above(
                    second_hinge.first_face,
                    second_hinge.second_face,
                    pair_values,
                )?,
                face_above(
                    second_hinge.first_face,
                    first_hinge.second_face,
                    pair_values,
                )?,
                face_above(
                    first_hinge.first_face,
                    second_hinge.second_face,
                    pair_values,
                )?,
                face_above(first_hinge.first_face, second_hinge.first_face, pair_values)?,
                face_above(
                    first_hinge.second_face,
                    second_hinge.second_face,
                    pair_values,
                )?,
            ];
            if !taco_taco_source_tuple_accepts(&relations) {
                return Err(certificate_failure());
            }
            expected_constraint_count = checked_certificate_count(expected_constraint_count, 1)?;
        }
    }

    if expected_constraint_count != problem.constraints.len() {
        return Err(certificate_failure());
    }
    Ok(())
}

fn checked_certificate_count(current: usize, additional: usize) -> FacewiseResult<usize> {
    current
        .checked_add(additional)
        .ok_or_else(certificate_failure)
}

fn all_face_pairs_assigned(faces: &[usize], pair_values: &PairValues) -> bool {
    (0..faces.len()).all(|first| {
        ((first + 1)..faces.len())
            .all(|second| pair_values.contains_key(&ordered_pair(faces[first], faces[second])))
    })
}

fn face_above(first: usize, second: usize, pair_values: &PairValues) -> FacewiseResult<bool> {
    let pair = ordered_pair(first, second);
    let canonical_second_above_first = pair_values
        .get(&pair)
        .copied()
        .ok_or_else(certificate_failure)?;
    Ok(if first == pair.0 {
        !canonical_second_above_first
    } else {
        canonical_second_above_first
    })
}

fn fresh_constraint_accepts(constraint: &TupleConstraint, assignment: &[bool]) -> bool {
    if constraint.variables.len() > 6
        || constraint
            .variables
            .iter()
            .any(|index| *index >= assignment.len())
        || constraint
            .variables
            .iter()
            .enumerate()
            .any(|(index, variable)| constraint.variables[..index].contains(variable))
    {
        return false;
    }
    let row = constraint
        .variables
        .iter()
        .enumerate()
        .fold(0_u8, |row, (position, variable)| {
            row | (u8::from(assignment[*variable]) << position)
        });
    constraint.allowed_rows.contains(&row)
}

struct CertificateByteCounter<'runtime, 'observer, O: GlobalFlatFoldabilityObserver + ?Sized> {
    runtime: &'runtime mut Runtime<'observer, O>,
    limit: usize,
    observed: usize,
    bytes_since_poll: usize,
    exceeded: bool,
    abort: Option<FacewiseAbort>,
}

impl<O: GlobalFlatFoldabilityObserver + ?Sized> Write for CertificateByteCounter<'_, '_, O> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(observed) = self.observed.checked_add(bytes.len()) else {
            self.observed = usize::MAX;
            self.exceeded = true;
            return Err(io::Error::other("certificate byte count overflowed"));
        };
        self.observed = observed;
        if self.observed > self.limit {
            self.exceeded = true;
            return Err(io::Error::other("certificate byte limit reached"));
        }
        self.bytes_since_poll = self.bytes_since_poll.saturating_add(bytes.len());
        if self.bytes_since_poll >= SERIALIZATION_POLL_BYTES {
            self.bytes_since_poll = 0;
            if let Err(abort) = self.runtime.checkpoint(None) {
                self.abort = Some(abort);
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "certificate serialization interrupted",
                ));
            }
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn serialized_certificate_size<O: GlobalFlatFoldabilityObserver + ?Sized>(
    layer_order: &LayerOrderSnapshot,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<usize> {
    runtime.checkpoint(None)?;
    let limit = runtime.limits.max_certificate_bytes;
    let serialization_result;
    let observed;
    let exceeded;
    let abort;
    {
        let mut counter = CertificateByteCounter {
            runtime,
            limit,
            observed: 0,
            bytes_since_poll: 0,
            exceeded: false,
            abort: None,
        };
        serialization_result = serde_json::to_writer(&mut counter, layer_order);
        observed = counter.observed;
        exceeded = counter.exceeded;
        abort = counter.abort.take();
    }
    if let Some(abort) = abort {
        return Err(abort);
    }
    if serialization_result.is_err() {
        if exceeded {
            return Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit,
                    observed,
                },
            ));
        }
        return Err(FacewiseAbort::Execution(internal_error()));
    }
    runtime.checkpoint(None)?;
    Ok(observed)
}

fn finalize_certificate_size<O: GlobalFlatFoldabilityObserver + ?Sized>(
    layer_order: &mut LayerOrderSnapshot,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<()> {
    let mut prior = 0_usize;
    for _ in 0..4 {
        let Some(summary) = layer_order.proof_summary.as_mut() else {
            return Err(FacewiseAbort::Execution(internal_error()));
        };
        summary.certificate_bytes = prior;
        let observed = serialized_certificate_size(layer_order, runtime)?;
        if observed == prior {
            runtime.set_certificate_bytes(observed)?;
            runtime.checkpoint(None)?;
            return Ok(());
        }
        prior = observed;
    }
    Err(FacewiseAbort::Execution(internal_error()))
}

const fn ordered_pair(first: usize, second: usize) -> (usize, usize) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

fn order_cell_faces<O: GlobalFlatFoldabilityObserver + ?Sized>(
    faces: &[usize],
    pair_values: &PairValues,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Vec<usize>> {
    let ordered_bytes = runtime.allocation_bytes(faces.len(), std::mem::size_of::<usize>())?;
    runtime.ensure_transient_exact_storage(ordered_bytes)?;
    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(faces.len())
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    ordered.extend_from_slice(faces);
    let mut control_poll = 0_usize;
    for index in 1..ordered.len() {
        let mut cursor = index;
        while cursor > 0 {
            runtime.poll_control(&mut control_poll)?;
            if !face_is_below(ordered[cursor], ordered[cursor - 1], pair_values)? {
                break;
            }
            ordered.swap(cursor, cursor - 1);
            cursor -= 1;
        }
    }
    runtime.checkpoint(None)?;
    Ok(ordered)
}

fn face_is_below(first: usize, second: usize, pair_values: &PairValues) -> FacewiseResult<bool> {
    let key = ordered_pair(first, second);
    let Some(second_canonical_above_first) = pair_values.get(&key).copied() else {
        return Err(FacewiseAbort::Unknown(
            GlobalFlatFoldabilityUnknownReason::ProofIncomplete {
                reason: FlatFoldabilityProofIncompleteReason::CertificateReverificationFailed,
            },
        ));
    };
    Ok(if first == key.0 {
        second_canonical_above_first
    } else {
        !second_canonical_above_first
    })
}

fn canonical_global_linear_extension<O: GlobalFlatFoldabilityObserver + ?Sized>(
    face_count: usize,
    pair_values: &PairValues,
    runtime: &mut Runtime<'_, O>,
) -> FacewiseResult<Option<Vec<usize>>> {
    runtime.checkpoint(None)?;
    let face_header_bytes = runtime.allocation_bytes(
        face_count,
        std::mem::size_of::<Vec<usize>>()
            .checked_add(3 * std::mem::size_of::<usize>())
            .and_then(|total| total.checked_add(std::mem::size_of::<u8>()))
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?,
    )?;
    let edge_bytes = runtime.allocation_bytes(pair_values.len(), std::mem::size_of::<usize>())?;
    let working_bytes = face_header_bytes
        .checked_add(edge_bytes)
        .and_then(|total| {
            total.checked_add(face_count.saturating_mul(2 * std::mem::size_of::<usize>()))
        })
        .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    runtime.ensure_transient_exact_storage(working_bytes)?;
    let mut outdegrees = Vec::new();
    outdegrees
        .try_reserve_exact(face_count)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    outdegrees.resize(face_count, 0_usize);
    for &((first, second), second_above_first) in pair_values.iter() {
        let lower = if second_above_first { first } else { second };
        outdegrees[lower] = outdegrees[lower]
            .checked_add(1)
            .ok_or_else(|| runtime.exact_storage_limit_failure(usize::MAX))?;
    }
    let mut outgoing = Vec::new();
    outgoing
        .try_reserve_exact(face_count)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    for degree in outdegrees {
        let mut neighbors = Vec::new();
        neighbors.try_reserve_exact(degree).map_err(|_| {
            runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes)
        })?;
        outgoing.push(neighbors);
    }
    let mut indegree = Vec::new();
    indegree
        .try_reserve_exact(face_count)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    indegree.resize(face_count, 0_usize);
    let mut control_poll = 0_usize;
    for &((first, second), second_above_first) in pair_values.iter() {
        runtime.poll_control(&mut control_poll)?;
        let (lower, upper) = if second_above_first {
            (first, second)
        } else {
            (second, first)
        };
        outgoing[lower].push(upper);
        indegree[upper] += 1;
    }
    for neighbors in &mut outgoing {
        neighbors.sort_unstable();
    }
    let mut ready = Vec::new();
    ready
        .try_reserve_exact(face_count)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    ready.extend(indegree.iter().map(|degree| *degree == 0));
    let mut result = Vec::new();
    result
        .try_reserve_exact(face_count)
        .map_err(|_| runtime.exact_storage_limit_failure(runtime.limits.max_certificate_bytes))?;
    while let Some(current) = (0..face_count).find(|index| ready[*index]) {
        runtime.poll_control(&mut control_poll)?;
        ready[current] = false;
        result.push(current);
        for upper in outgoing[current].iter().copied() {
            runtime.poll_control(&mut control_poll)?;
            indegree[upper] -= 1;
            if indegree[upper] == 0 {
                ready[upper] = true;
            }
        }
    }
    if result.len() != face_count {
        // Facewise orders are location-dependent. A cycle assembled from
        // disjoint overlap cells is not a physical contradiction.
        runtime.checkpoint(None)?;
        return Ok(None);
    }
    runtime.checkpoint(None)?;
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopGlobalFlatFoldabilityObserver;
    use ori_domain::{CreasePattern, Edge, EdgeKind, Paper, Point2, ProjectId, Vertex, VertexId};
    use ori_topology::{FaceExtractionInput, extract_faces_strict};
    use serde::de::DeserializeOwned;

    fn fixed_id<T: DeserializeOwned>(suffix: u64) -> T {
        serde_json::from_str(&format!("\"00000000-0000-0000-0000-{suffix:012x}\""))
            .expect("fixed UUID fixture")
    }

    fn zero_work() -> GlobalFlatFoldabilityWorkCounts {
        GlobalFlatFoldabilityWorkCounts {
            source_vertex_records: 0,
            source_edge_records: 0,
            paper_boundary_vertex_records: 0,
            face_records: 0,
            face_boundary_half_edges: 0,
            hinge_records: 0,
            edge_incidence_records: 0,
            local_vertex_records: 0,
            total_records: 0,
            overlap_face_pairs: 0,
            arrangement_segments: 0,
            overlap_cells: 0,
            constraints: 0,
            search_nodes: 0,
            exact_operations: 0,
            exact_values: 0,
            certificate_bytes: 0,
        }
    }

    struct DeadlineAfter {
        continued_checkpoints: usize,
    }

    impl GlobalFlatFoldabilityObserver for DeadlineAfter {
        fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
            if self.continued_checkpoints == 0 {
                GlobalFlatFoldabilityCheckpoint::DeadlineReached
            } else {
                self.continued_checkpoints -= 1;
                GlobalFlatFoldabilityCheckpoint::Continue
            }
        }
    }

    struct AlwaysCancel;

    impl GlobalFlatFoldabilityObserver for AlwaysCancel {
        fn checkpoint(&mut self) -> GlobalFlatFoldabilityCheckpoint {
            GlobalFlatFoldabilityCheckpoint::Cancelled
        }
    }

    fn integer_point(x: i64, y: i64) -> Point {
        Point {
            x: Rational::from_integer(x.into()),
            y: Rational::from_integer(y.into()),
        }
    }

    fn three_panel_accordion() -> (Paper, CreasePattern, TopologySnapshot) {
        let vertices = (0..8)
            .map(|index| fixed_id::<VertexId>(0x100 + index))
            .collect::<Vec<_>>();
        let positions = [
            Point2::new(0.0, 0.0),
            Point2::new(2.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(6.0, 0.0),
            Point2::new(6.0, 2.0),
            Point2::new(4.0, 2.0),
            Point2::new(2.0, 2.0),
            Point2::new(0.0, 2.0),
        ];
        let vertex_records = vertices
            .iter()
            .copied()
            .zip(positions)
            .map(|(id, position)| Vertex { id, position })
            .collect::<Vec<_>>();
        let mut edges = (0..vertices.len())
            .map(|index| Edge {
                id: fixed_id(0x200 + index as u64),
                start: vertices[index],
                end: vertices[(index + 1) % vertices.len()],
                kind: EdgeKind::Boundary,
            })
            .collect::<Vec<_>>();
        edges.push(Edge {
            id: fixed_id(0x301),
            start: vertices[1],
            end: vertices[6],
            kind: EdgeKind::Mountain,
        });
        edges.push(Edge {
            id: fixed_id(0x302),
            start: vertices[2],
            end: vertices[5],
            kind: EdgeKind::Valley,
        });
        let paper = Paper {
            boundary_vertices: vertices,
            ..Paper::default()
        };
        let pattern = CreasePattern {
            vertices: vertex_records,
            edges,
        };
        let topology = extract_faces_strict(FaceExtractionInput {
            identity_namespace: fixed_id::<ProjectId>(1),
            source_revision: 73,
            paper: &paper,
            pattern: &pattern,
        })
        .expect("three-panel accordion topology");
        (paper, pattern, topology)
    }

    fn synthetic_face(index: usize, polygon: Vec<Point>, front_up: bool) -> FoldedFace {
        let layer = LayerFace {
            face_id: fixed_id(0x900 + index as u64),
            face_key: ori_topology::FaceKey([u8::try_from(index + 1).unwrap_or(255); 32]),
        };
        FoldedFace {
            source: SourceFace {
                layer,
                vertex_ids: Vec::new(),
                source_polygon: polygon.clone(),
            },
            transform: Transform::identity(),
            front_up,
            polygon,
        }
    }

    fn synthetic_cell(key: u8, boundary: Vec<Point>, covering_faces: Vec<usize>) -> OverlapCell {
        OverlapCell {
            key: OverlapCellKey([key; 32]),
            boundary,
            covering_faces,
        }
    }

    fn all_pairs(face_count: usize) -> Vec<OverlapPair> {
        (0..face_count)
            .flat_map(|first| {
                ((first + 1)..face_count).map(move |second| OverlapPair { first, second })
            })
            .collect()
    }

    #[test]
    fn taco_taco_compiler_matches_all_fixed_source_rows_after_direction_reversal() {
        let (a, b, c, d) = (3_usize, 0_usize, 2_usize, 1_usize);
        let directed_relations = [(a, b), (c, d), (c, b), (a, d), (a, c), (b, d)];
        let variables = [(0_usize, 1_usize), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        let constraint = match relation_constraint(
            RelationConstraintInput {
                kind: FacewiseConstraintKind::TacoTaco,
                relations: &directed_relations,
                faces: &[a, b, c, d],
                supporting_cell: None,
                variable_pairs: &variables,
            },
            taco_taco_source_tuple_accepts,
            &runtime,
            ConstraintStorageScope::Primary,
        ) {
            Ok(constraint) => constraint,
            Err(_) => panic!("six-relation taco-taco constraint compiles"),
        };
        assert_eq!(constraint.variables, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(constraint.allowed_rows.len(), 16);
        for canonical_row in 0_u8..64 {
            let directed = directed_relations
                .iter()
                .map(|&(first, second)| {
                    match directed_face_above_from_row(
                        first,
                        second,
                        canonical_row,
                        &constraint.variables,
                        &variables,
                    ) {
                        Ok(value) => value,
                        Err(_) => panic!("every directed pair maps to a canonical variable"),
                    }
                })
                .collect::<Vec<_>>();
            assert_eq!(
                constraint.allowed_rows.contains(&canonical_row),
                taco_taco_source_tuple_accepts(&directed),
                "canonical assignment row {canonical_row:06b}"
            );
        }
    }

    #[test]
    fn source_taco_taco_table_rejects_sum_only_counterexamples() {
        assert!(!taco_taco_source_tuple_accepts(&[
            true, true, false, true, true, false,
        ]));
        assert!(!taco_taco_source_tuple_accepts(&[
            true, true, true, false, false, true,
        ]));
    }

    #[test]
    fn taco_taco_table_is_invariant_under_swapping_each_taco() {
        for canonical_row in 0_u8..64 {
            let mut pair_values = PairValues::default();
            for (position, pair) in [(0_usize, 1_usize), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]
                .into_iter()
                .enumerate()
            {
                pair_values.insert(pair, canonical_row & (1 << position) != 0);
            }
            let tuple = |a, b, c, d| {
                [
                    face_above(a, b, &pair_values).unwrap_or(false),
                    face_above(c, d, &pair_values).unwrap_or(false),
                    face_above(c, b, &pair_values).unwrap_or(false),
                    face_above(a, d, &pair_values).unwrap_or(false),
                    face_above(a, c, &pair_values).unwrap_or(false),
                    face_above(b, d, &pair_values).unwrap_or(false),
                ]
            };
            let expected = taco_taco_source_tuple_accepts(&tuple(0, 1, 2, 3));
            assert_eq!(taco_taco_source_tuple_accepts(&tuple(1, 0, 2, 3)), expected);
            assert_eq!(taco_taco_source_tuple_accepts(&tuple(0, 1, 3, 2)), expected);
        }
    }

    #[test]
    fn two_and_three_relation_templates_match_the_source_truth_tables() {
        let variables = [(0_usize, 1_usize), (0, 2), (1, 2)];
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        let transitivity = match relation_constraint(
            RelationConstraintInput {
                kind: FacewiseConstraintKind::Transitivity,
                relations: &[(0, 1), (1, 2), (2, 0)],
                faces: &[0, 1, 2],
                supporting_cell: None,
                variable_pairs: &variables,
            },
            |relations| !(relations[0] == relations[1] && relations[1] == relations[2]),
            &runtime,
            ConstraintStorageScope::Primary,
        ) {
            Ok(value) => value,
            Err(_) => panic!("transitivity compiles"),
        };
        assert_eq!(transitivity.allowed_rows.len(), 6);
        let taco_tortilla = match relation_constraint(
            RelationConstraintInput {
                kind: FacewiseConstraintKind::TacoTortilla,
                relations: &[(0, 2), (1, 2)],
                faces: &[0, 1, 2],
                supporting_cell: None,
                variable_pairs: &variables,
            },
            |relations| relations[0] == relations[1],
            &runtime,
            ConstraintStorageScope::Primary,
        ) {
            Ok(value) => value,
            Err(_) => panic!("taco-tortilla compiles"),
        };
        let tortilla_tortilla = match relation_constraint(
            RelationConstraintInput {
                kind: FacewiseConstraintKind::TortillaTortilla,
                relations: &[(0, 2), (1, 2)],
                faces: &[0, 1, 2],
                supporting_cell: None,
                variable_pairs: &variables,
            },
            |relations| relations[0] == relations[1],
            &runtime,
            ConstraintStorageScope::Primary,
        ) {
            Ok(value) => value,
            Err(_) => panic!("tortilla-tortilla compiles"),
        };
        assert_eq!(taco_tortilla.allowed_rows.len(), 2);
        assert_eq!(taco_tortilla.allowed_rows, tortilla_tortilla.allowed_rows);
    }

    #[test]
    fn exact_segment_classification_is_open_and_positive_length_only() {
        let square = vec![
            integer_point(-1, -1),
            integer_point(1, -1),
            integer_point(1, 1),
            integer_point(-1, 1),
        ];
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        assert!(
            segment_overlaps_face_interior(
                &integer_point(-2, 0),
                &integer_point(2, 0),
                &square,
                &mut runtime,
            )
            .unwrap_or(false)
        );
        assert!(
            !segment_overlaps_face_interior(
                &integer_point(-2, 0),
                &integer_point(-1, 0),
                &square,
                &mut runtime,
            )
            .unwrap_or(true)
        );
        assert!(
            !segment_overlaps_face_interior(
                &integer_point(-1, -1),
                &integer_point(1, -1),
                &square,
                &mut runtime,
            )
            .unwrap_or(true)
        );
        assert!(
            segments_overlap_in_positive_length(
                &integer_point(0, 0),
                &integer_point(2, 0),
                &integer_point(1, 0),
                &integer_point(3, 0),
                &mut runtime,
            )
            .unwrap_or(false)
        );
        assert!(
            !segments_overlap_in_positive_length(
                &integer_point(0, 0),
                &integer_point(2, 0),
                &integer_point(2, 0),
                &integer_point(3, 0),
                &mut runtime,
            )
            .unwrap_or(true)
        );
        assert!(
            !segments_overlap_in_positive_length(
                &integer_point(0, 0),
                &integer_point(2, 0),
                &integer_point(1, -1),
                &integer_point(1, 1),
                &mut runtime,
            )
            .unwrap_or(true)
        );
    }

    #[test]
    fn geometry_enumerates_taco_tortilla_and_same_side_taco_taco_only() {
        let upper = vec![
            integer_point(-2, 0),
            integer_point(2, 0),
            integer_point(2, 2),
            integer_point(-2, 2),
        ];
        let crossing = vec![
            integer_point(-1, -1),
            integer_point(1, -1),
            integer_point(1, 1),
            integer_point(-1, 1),
        ];
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        let taco_tortilla_embedding = FlatEmbedding {
            reference_face: 0,
            faces: vec![
                synthetic_face(0, upper.clone(), true),
                synthetic_face(1, upper.clone(), false),
                synthetic_face(2, crossing, true),
            ],
            hinges: vec![FoldedHinge {
                edge: fixed_id(0xa01),
                first_face: 0,
                second_face: 1,
                assignment: FoldAssignment::Mountain,
                first_point: integer_point(-1, 0),
                second_point: integer_point(1, 0),
            }],
            material_internal_edge_count: 1,
        };
        let taco_tortilla_cells = vec![synthetic_cell(
            1,
            vec![
                integer_point(-1, 0),
                integer_point(1, 0),
                integer_point(1, 1),
                integer_point(-1, 1),
            ],
            vec![0, 1, 2],
        )];
        let problem = build_constraint_problem(
            &taco_tortilla_embedding,
            &all_pairs(3),
            &taco_tortilla_cells,
            &mut runtime,
            true,
        )
        .expect("hinge crossing a third face builds constraints");
        assert_eq!(
            problem
                .constraints
                .iter()
                .filter(|constraint| constraint.kind == FacewiseConstraintKind::TacoTortilla)
                .count(),
            1
        );

        let same_side_embedding = FlatEmbedding {
            reference_face: 0,
            faces: vec![
                synthetic_face(0, upper.clone(), true),
                synthetic_face(1, upper.clone(), false),
                synthetic_face(2, upper.clone(), true),
                synthetic_face(3, upper.clone(), false),
            ],
            hinges: vec![
                FoldedHinge {
                    edge: fixed_id(0xa11),
                    first_face: 0,
                    second_face: 1,
                    assignment: FoldAssignment::Mountain,
                    first_point: integer_point(-1, 0),
                    second_point: integer_point(1, 0),
                },
                FoldedHinge {
                    edge: fixed_id(0xa12),
                    first_face: 2,
                    second_face: 3,
                    assignment: FoldAssignment::Valley,
                    first_point: integer_point(-1, 0),
                    second_point: integer_point(1, 0),
                },
            ],
            material_internal_edge_count: 2,
        };
        let same_side_cells = vec![synthetic_cell(2, upper.clone(), vec![0, 1, 2, 3])];
        let same_side = build_constraint_problem(
            &same_side_embedding,
            &all_pairs(4),
            &same_side_cells,
            &mut runtime,
            true,
        )
        .expect("same-side tacos build constraints");
        assert_eq!(
            same_side
                .constraints
                .iter()
                .filter(|constraint| constraint.kind == FacewiseConstraintKind::TacoTaco)
                .count(),
            1
        );

        let lower = vec![
            integer_point(-2, -2),
            integer_point(2, -2),
            integer_point(2, 0),
            integer_point(-2, 0),
        ];
        let opposite_embedding = FlatEmbedding {
            reference_face: 0,
            faces: vec![
                synthetic_face(0, upper.clone(), true),
                synthetic_face(1, upper.clone(), false),
                synthetic_face(2, lower.clone(), true),
                synthetic_face(3, lower.clone(), false),
            ],
            hinges: same_side_embedding.hinges,
            material_internal_edge_count: 2,
        };
        let opposite_pairs = vec![
            OverlapPair {
                first: 0,
                second: 1,
            },
            OverlapPair {
                first: 2,
                second: 3,
            },
        ];
        let opposite_cells = vec![
            synthetic_cell(3, upper, vec![0, 1]),
            synthetic_cell(4, lower, vec![2, 3]),
        ];
        let opposite = build_constraint_problem(
            &opposite_embedding,
            &opposite_pairs,
            &opposite_cells,
            &mut runtime,
            true,
        )
        .expect("opposite-side tacos remain independently ordered");
        assert_eq!(
            opposite
                .constraints
                .iter()
                .filter(|constraint| constraint.kind == FacewiseConstraintKind::TacoTaco)
                .count(),
            0
        );
    }

    #[test]
    fn mountain_valley_fixing_covers_orientation_and_hinge_order() {
        assert!(mountain_valley_canonical_value(
            FoldAssignment::Mountain,
            true,
            0,
            1
        ));
        assert!(!mountain_valley_canonical_value(
            FoldAssignment::Valley,
            true,
            0,
            1
        ));
        assert!(!mountain_valley_canonical_value(
            FoldAssignment::Mountain,
            false,
            0,
            1
        ));
        assert!(mountain_valley_canonical_value(
            FoldAssignment::Valley,
            false,
            0,
            1
        ));
        assert!(!mountain_valley_canonical_value(
            FoldAssignment::Mountain,
            true,
            1,
            0
        ));
    }

    #[test]
    fn disjoint_cell_cycle_has_no_global_linearization_but_local_orders_remain_valid() {
        let mut pair_values = PairValues::default();
        pair_values.insert((0, 1), true);
        pair_values.insert((1, 2), true);
        pair_values.insert((0, 2), false);
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        assert_eq!(
            canonical_global_linear_extension(3, &pair_values, &mut runtime)
                .expect("global extension check"),
            None
        );
        for faces in [[0_usize, 1_usize], [1, 2], [0, 2]] {
            assert!(order_cell_faces(&faces, &pair_values, &mut runtime).is_ok());
        }
    }

    #[test]
    fn three_panel_geometry_builds_and_reverifies_a_facewise_certificate() {
        let (paper, pattern, topology) = three_panel_accordion();
        let canonical_faces = topology
            .faces
            .iter()
            .map(|face| LayerFace {
                face_id: face.id,
                face_key: face.key,
            })
            .collect::<Vec<_>>();
        let provenance = GlobalFlatFoldabilityProvenance {
            identity_namespace: Some(fixed_id(1)),
            source_revision: topology.source_revision,
            source_fingerprint: Some(crate::fold_model_fingerprint_v1(&pattern, &paper)),
            model_id: crate::GLOBAL_FLAT_FOLDABILITY_MODEL_ID,
        };
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        runtime
            .advance(
                GlobalFlatFoldabilityPhase::BuildingFlatEmbedding,
                Some(canonical_faces.len()),
            )
            .expect("embedding phase");
        let embedding =
            build_flat_embedding(&paper, &pattern, &topology, &canonical_faces, &mut runtime)
                .expect("exact accordion embedding");
        assert_eq!(embedding.faces.len(), 3);
        runtime
            .advance(GlobalFlatFoldabilityPhase::BuildingOverlapArrangement, None)
            .expect("arrangement phase");
        let pairs = build_overlap_pairs(&embedding.faces, &mut runtime).expect("overlap pairs");
        let cells =
            build_overlap_cells(&embedding.faces, &pairs, &mut runtime).expect("atomic cells");
        let expected_pair_count = pairs.len();
        assert_eq!(pairs.len(), 3);
        assert_eq!(runtime.work.overlap_face_pairs, expected_pair_count);
        assert!(cells.iter().any(|cell| cell.covering_faces.len() == 3));
        assert_eq!(cells.len(), 1);
        let original_cell = &cells[0];
        let min_x = original_cell
            .boundary
            .iter()
            .map(|point| point.x.clone())
            .min()
            .expect("cell minimum x");
        let max_x = original_cell
            .boundary
            .iter()
            .map(|point| point.x.clone())
            .max()
            .expect("cell maximum x");
        let min_y = original_cell
            .boundary
            .iter()
            .map(|point| point.y.clone())
            .min()
            .expect("cell minimum y");
        let max_y = original_cell
            .boundary
            .iter()
            .map(|point| point.y.clone())
            .max()
            .expect("cell maximum y");
        let split_x = (&min_x + &max_x) / Rational::from_integer(2.into());
        let split_first = Point {
            x: split_x.clone(),
            y: min_y,
        };
        let split_second = Point {
            x: split_x,
            y: max_y,
        };
        let first_boundary = clip_polygon_halfplane(
            &original_cell.boundary,
            &split_first,
            &split_second,
            true,
            0,
            &mut runtime,
        )
        .expect("left atomic half");
        let second_boundary = clip_polygon_halfplane(
            &original_cell.boundary,
            &split_first,
            &split_second,
            false,
            exact_storage_bytes_points(&first_boundary).expect("first half bytes"),
            &mut runtime,
        )
        .expect("right atomic half");
        let cells = [first_boundary, second_boundary]
            .into_iter()
            .map(|boundary| {
                let key = overlap_cell_key(
                    &boundary,
                    &original_cell.covering_faces,
                    &embedding.faces,
                    &mut runtime,
                )
                .expect("derived split-cell key");
                OverlapCell {
                    key,
                    boundary,
                    covering_faces: original_cell.covering_faces.clone(),
                }
            })
            .collect::<Vec<_>>();
        runtime
            .set_overlap_cells(cells.len())
            .expect("two split cells fit");
        runtime
            .set_arrangement_exact_storage(
                cells
                    .iter()
                    .map(|cell| {
                        exact_storage_bytes_points(&cell.boundary).expect("cell boundary bytes")
                    })
                    .fold(0_usize, usize::saturating_add),
            )
            .expect("split-cell exact storage fits");
        let success = solve_layer_order(
            embedding.clone(),
            pairs,
            cells.clone(),
            provenance,
            &mut runtime,
        )
        .expect("accordion has a verified layer order");
        assert_eq!(success.layer_order.material_faces.len(), 3);
        assert_eq!(success.layer_order.overlap_cells.len(), cells.len());
        assert_eq!(success.layer_order.face_pair_orders.len(), 3);
        let proof_summary = success
            .layer_order
            .proof_summary
            .expect("verified proof summary");
        assert_eq!(proof_summary.overlap_face_pairs, expected_pair_count);
        assert_eq!(proof_summary.constraints, runtime.work.constraints);

        let face_indexes = embedding
            .faces
            .iter()
            .enumerate()
            .map(|(index, face)| (face.source.layer.face_id, index))
            .collect::<HashMap<_, _>>();
        let mut pair_values = PairValues::default();
        for order in &success.layer_order.face_pair_orders {
            let lower = face_indexes[&order.lower_face.face_id];
            let upper = face_indexes[&order.upper_face.face_id];
            let pair = ordered_pair(lower, upper);
            pair_values.insert(pair, upper == pair.1);
        }
        verify_layer_order_snapshot(
            &success.layer_order,
            &embedding,
            &cells,
            &pair_values,
            provenance,
            &mut runtime,
        )
        .expect("untampered snapshot reverifies");

        let untampered = success.layer_order;
        let actual_certificate_bytes = untampered
            .proof_summary
            .expect("certificate summary")
            .certificate_bytes;
        let mut exact_limit_observer = NoopGlobalFlatFoldabilityObserver;
        let mut exact_limit_runtime = Runtime::new(
            &mut exact_limit_observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: actual_certificate_bytes,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        assert_eq!(
            serialized_certificate_size(&untampered, &mut exact_limit_runtime)
                .expect("real certificate fits its exact serialized size"),
            actual_certificate_bytes
        );
        let mut one_byte_short_observer = NoopGlobalFlatFoldabilityObserver;
        let mut one_byte_short_runtime = Runtime::new(
            &mut one_byte_short_observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: actual_certificate_bytes - 1,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        assert!(matches!(
            serialized_certificate_size(&untampered, &mut one_byte_short_runtime),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit,
                    observed,
                }
            )) if limit == actual_certificate_bytes - 1
                && observed == actual_certificate_bytes
        ));
        let mut deadline_observer = DeadlineAfter {
            continued_checkpoints: 1,
        };
        let mut deadline_runtime = Runtime::new(
            &mut deadline_observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        deadline_runtime.phase = GlobalFlatFoldabilityPhase::VerifyingCertificate;
        assert!(matches!(
            serialized_certificate_size(&untampered, &mut deadline_runtime),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::TimeLimitReached {
                    phase: GlobalFlatFoldabilityPhase::VerifyingCertificate,
                }
            ))
        ));

        let mut tampered = untampered.clone();
        let cell = tampered
            .overlap_cells
            .iter_mut()
            .find(|cell| cell.bottom_to_top_faces.len() >= 2)
            .expect("multi-layer cell");
        cell.bottom_to_top_faces.swap(0, 1);
        assert!(
            verify_layer_order_snapshot(
                &tampered,
                &embedding,
                &cells,
                &pair_values,
                provenance,
                &mut runtime,
            )
            .is_err()
        );

        assert!(untampered.overlap_cells.len() >= 2);
        let mut duplicate_cell = untampered.clone();
        duplicate_cell.overlap_cells[1] = duplicate_cell.overlap_cells[0].clone();
        assert!(
            verify_layer_order_snapshot(
                &duplicate_cell,
                &embedding,
                &cells,
                &pair_values,
                provenance,
                &mut runtime,
            )
            .is_err()
        );

        let mut forged_internal_cells = cells.clone();
        let forged_key = OverlapCellKey([0xa5; 32]);
        let original_key = forged_internal_cells[0].key;
        forged_internal_cells[0].key = forged_key;
        let mut forged_internal_and_snapshot_key = untampered.clone();
        forged_internal_and_snapshot_key
            .overlap_cells
            .iter_mut()
            .find(|cell| cell.cell_key == original_key)
            .expect("corresponding overlap-cell snapshot")
            .cell_key = forged_key;
        assert!(
            verify_layer_order_snapshot(
                &forged_internal_and_snapshot_key,
                &embedding,
                &forged_internal_cells,
                &pair_values,
                provenance,
                &mut runtime,
            )
            .is_err()
        );

        let mut reordered_cells = untampered.clone();
        reordered_cells.overlap_cells.swap(0, 1);
        assert!(
            verify_layer_order_snapshot(
                &reordered_cells,
                &embedding,
                &cells,
                &pair_values,
                provenance,
                &mut runtime,
            )
            .is_err()
        );

        assert!(untampered.face_pair_orders.len() >= 2);
        let mut duplicate_pair = untampered.clone();
        duplicate_pair.face_pair_orders[1] = duplicate_pair.face_pair_orders[0].clone();
        assert!(
            verify_layer_order_snapshot(
                &duplicate_pair,
                &embedding,
                &cells,
                &pair_values,
                provenance,
                &mut runtime,
            )
            .is_err()
        );

        let mut reordered_pairs = untampered.clone();
        reordered_pairs.face_pair_orders.swap(0, 1);
        assert!(
            verify_layer_order_snapshot(
                &reordered_pairs,
                &embedding,
                &cells,
                &pair_values,
                provenance,
                &mut runtime,
            )
            .is_err()
        );

        let mut tampered_derivation = untampered.clone();
        let LayerOrderDerivation::FacewiseCertificate {
            overlap_cell_count, ..
        } = &mut tampered_derivation.provenance.derivation
        else {
            panic!("three panels use a facewise certificate derivation");
        };
        *overlap_cell_count += 1;
        assert!(
            verify_layer_order_snapshot(
                &tampered_derivation,
                &embedding,
                &cells,
                &pair_values,
                provenance,
                &mut runtime,
            )
            .is_err()
        );

        let original_certificate_bytes = runtime.work.certificate_bytes;
        let forged_certificate_bytes = original_certificate_bytes + 1;
        let mut tampered_bytes = untampered.clone();
        tampered_bytes
            .proof_summary
            .as_mut()
            .expect("facewise proof summary")
            .certificate_bytes = forged_certificate_bytes;
        runtime.work.certificate_bytes = forged_certificate_bytes;
        assert!(
            verify_layer_order_snapshot(
                &tampered_bytes,
                &embedding,
                &cells,
                &pair_values,
                provenance,
                &mut runtime,
            )
            .is_err()
        );
    }

    #[test]
    fn exact_storage_budget_admits_128_mib_and_rejects_one_more_byte() {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        runtime
            .set_embedding_exact_storage(crate::DEFAULT_MAX_CERTIFICATE_BYTES)
            .expect("the exact 128 MiB boundary is admitted");
        runtime
            .ensure_transient_exact_storage(0)
            .expect("zero additional bytes remain admitted");
        assert!(matches!(
            runtime.ensure_transient_exact_storage(1),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit: crate::DEFAULT_MAX_CERTIFICATE_BYTES,
                    observed,
                }
            )) if observed == crate::DEFAULT_MAX_CERTIFICATE_BYTES + 1
        ));
    }

    #[test]
    fn storage_arithmetic_overflow_fails_closed_even_with_usize_max_limit() {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: usize::MAX,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        assert!(matches!(
            runtime.set_embedding_exact_storage(usize::MAX),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit: usize::MAX,
                    observed: usize::MAX,
                }
            ))
        ));
        runtime.exact_storage.certificate_structure_bytes = usize::MAX - 1;
        assert!(matches!(
            runtime.add_certificate_structure_storage(2),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit: usize::MAX,
                    observed: usize::MAX,
                }
            ))
        ));
    }

    #[test]
    fn certificate_structure_and_verifier_storage_share_one_live_memory_limit() {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: 128,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        runtime
            .add_certificate_structure_storage(80)
            .expect("retained certificate structure fits");
        runtime
            .add_verification_storage(48)
            .expect("live verifier reconstruction fits at equality");
        assert_eq!(runtime.exact_storage.total(), Some(128));
        assert!(matches!(
            runtime.add_verification_storage(1),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit: 128,
                    observed: 129,
                }
            ))
        ));
        assert_eq!(
            runtime.verification_storage_bytes(),
            48,
            "a rejected allocation is not committed to the live budget"
        );
        runtime.restore_verification_storage(0);
        runtime
            .ensure_transient_exact_storage(48)
            .expect("released verifier storage becomes available to a scoped value");
    }

    #[test]
    fn retained_constraint_problem_and_regenerated_verifier_share_one_live_limit() {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: 128,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        runtime
            .add_constraint_storage(80)
            .expect("primary constraint problem fits");
        runtime
            .add_verification_storage(48)
            .expect("regenerated verifier fits at the shared equality boundary");
        assert_eq!(runtime.exact_storage.total(), Some(128));
        assert!(matches!(
            runtime.add_verification_storage(1),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit: 128,
                    observed: 129,
                }
            ))
        ));
        runtime.restore_verification_storage(0);
        runtime
            .ensure_constraint_transient_storage(48)
            .expect("verification scope release restores the primary problem headroom");
        runtime.clear_constraint_storage();
        assert_eq!(runtime.exact_storage.total(), Some(0));
    }

    #[test]
    fn retained_solver_assignment_remains_charged_until_pair_values_replace_it() {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: 128,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        runtime
            .add_constraint_storage(80)
            .expect("retained constraint problem fits");
        runtime
            .add_constraint_storage(48)
            .expect("returned assignment fits at equality");
        assert_eq!(runtime.exact_storage.total(), Some(128));
        assert!(matches!(
            runtime.add_certificate_structure_storage(1),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit: 128,
                    observed: 129,
                }
            ))
        ));
        runtime.clear_constraint_storage();
        runtime
            .add_certificate_structure_storage(128)
            .expect("assignment and problem storage are released together after both values drop");
    }

    #[test]
    fn tuple_constraint_storage_accepts_exact_limit_and_rejects_one_byte_less_before_outer_alloc() {
        fn fixture() -> TupleConstraint {
            TupleConstraint {
                kind: FacewiseConstraintKind::Transitivity,
                variables: vec![0, 1, 2],
                allowed_rows: vec![0, 1, 2, 4, 5, 6],
                faces: vec![0, 1, 2],
                supporting_cell: None,
            }
        }
        let sizing = fixture();
        let nested = sizing.variables.capacity() * std::mem::size_of::<usize>()
            + sizing.allowed_rows.capacity() * std::mem::size_of::<u8>()
            + sizing.faces.capacity() * std::mem::size_of::<usize>();
        let exact_limit = nested + 4 * std::mem::size_of::<TupleConstraint>();

        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_constraints: 5_000_000,
                max_certificate_bytes: exact_limit,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        let mut constraints = Vec::new();
        push_constraint(
            &mut constraints,
            sizing,
            &mut runtime,
            ConstraintStorageScope::Primary,
        )
        .expect("exact tuple and outer Vec storage boundary is admitted");
        assert_eq!(runtime.exact_storage.total(), Some(exact_limit));

        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut over_limit = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_constraints: 5_000_000,
                max_certificate_bytes: exact_limit - 1,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        let mut rejected = Vec::new();
        assert!(matches!(
            push_constraint(
                &mut rejected,
                fixture(),
                &mut over_limit,
                ConstraintStorageScope::Primary,
            ),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit,
                    observed,
                }
            )) if limit == exact_limit - 1 && observed == exact_limit
        ));
        assert_eq!(
            rejected.capacity(),
            0,
            "the large outer Vec is not allocated"
        );
        assert_eq!(over_limit.exact_storage.constraint_bytes, 0);
    }

    #[test]
    fn tuple_constraint_outer_growth_accounts_old_and_new_buffers_at_the_reallocation_peak() {
        fn fixture() -> TupleConstraint {
            TupleConstraint {
                kind: FacewiseConstraintKind::Antisymmetry,
                variables: vec![0],
                allowed_rows: vec![0, 1],
                faces: vec![0, 1],
                supporting_cell: None,
            }
        }

        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        let mut constraints = Vec::new();
        for _ in 0..4 {
            push_constraint(
                &mut constraints,
                fixture(),
                &mut runtime,
                ConstraintStorageScope::Primary,
            )
            .expect("initial four-entry buffer");
        }
        assert_eq!(constraints.capacity(), 4);
        let retained_before = runtime.exact_storage.constraint_bytes;
        let next = fixture();
        let nested = next.variables.capacity() * std::mem::size_of::<usize>()
            + next.allowed_rows.capacity() * std::mem::size_of::<u8>()
            + next.faces.capacity() * std::mem::size_of::<usize>();
        let outer_delta = 4 * std::mem::size_of::<TupleConstraint>();
        let old_outer = 4 * std::mem::size_of::<TupleConstraint>();
        let peak = retained_before + nested + outer_delta + old_outer;
        runtime.limits.max_certificate_bytes = peak - 1;
        assert!(matches!(
            push_constraint(
                &mut constraints,
                next,
                &mut runtime,
                ConstraintStorageScope::Primary,
            ),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit,
                    observed,
                }
            )) if limit == peak - 1 && observed == peak
        ));
        assert_eq!(constraints.len(), 4);
        assert_eq!(constraints.capacity(), 4);
        assert_eq!(runtime.exact_storage.constraint_bytes, retained_before);

        runtime.limits.max_certificate_bytes = peak;
        push_constraint(
            &mut constraints,
            fixture(),
            &mut runtime,
            ConstraintStorageScope::Primary,
        )
        .expect("the exact old-plus-new reallocation peak is admitted");
        assert_eq!(constraints.len(), 5);
    }

    #[test]
    fn constraint_storage_overflow_is_fail_closed() {
        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: usize::MAX,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        runtime.exact_storage.constraint_bytes = usize::MAX - 1;
        assert!(matches!(
            runtime.add_constraint_storage(2),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit: usize::MAX,
                    observed: usize::MAX,
                }
            ))
        ));
    }

    #[test]
    fn deadline_and_cancel_are_observed_before_constraint_outer_allocation() {
        fn fixture() -> TupleConstraint {
            TupleConstraint {
                kind: FacewiseConstraintKind::Antisymmetry,
                variables: vec![0],
                allowed_rows: vec![0, 1],
                faces: vec![0, 1],
                supporting_cell: None,
            }
        }

        let mut deadline_observer = DeadlineAfter {
            continued_checkpoints: 0,
        };
        let mut deadline_runtime = Runtime::new(
            &mut deadline_observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        let mut constraints = Vec::new();
        assert!(matches!(
            push_constraint(
                &mut constraints,
                fixture(),
                &mut deadline_runtime,
                ConstraintStorageScope::Primary,
            ),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::TimeLimitReached { .. }
            ))
        ));
        assert_eq!(constraints.capacity(), 0);
        assert_eq!(deadline_runtime.exact_storage.constraint_bytes, 0);

        let mut cancel_observer = AlwaysCancel;
        let mut cancel_runtime = Runtime::new(
            &mut cancel_observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        let mut constraints = Vec::new();
        assert!(matches!(
            push_constraint(
                &mut constraints,
                fixture(),
                &mut cancel_runtime,
                ConstraintStorageScope::Primary,
            ),
            Err(FacewiseAbort::Execution(
                GlobalFlatFoldabilityExecutionError::Cancelled
            ))
        ));
        assert_eq!(constraints.capacity(), 0);
        assert_eq!(cancel_runtime.exact_storage.constraint_bytes, 0);
    }

    #[test]
    fn arrangement_cell_boundary_and_snapshot_exact_storage_are_aggregated() {
        let embedding_points = vec![integer_point(-2, -2), integer_point(2, -2)];
        let cell_boundary = vec![
            integer_point(-1, -1),
            integer_point(1, -1),
            integer_point(1, 1),
            integer_point(-1, 1),
        ];
        let embedding_bytes =
            exact_storage_bytes_points(&embedding_points).expect("embedding bytes");
        let boundary_bytes =
            exact_storage_bytes_points(&cell_boundary).expect("cell boundary bytes");
        let snapshot_bytes = boundary_bytes;
        let exact_limit = embedding_bytes + boundary_bytes + snapshot_bytes;

        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: exact_limit,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        runtime
            .set_embedding_exact_storage(embedding_bytes)
            .expect("embedding storage fits");
        runtime
            .set_arrangement_exact_storage(boundary_bytes)
            .expect("cell boundary storage fits");
        runtime
            .add_snapshot_exact_storage(snapshot_bytes)
            .expect("aggregate equality is admitted");
        assert_eq!(runtime.exact_storage.total(), Some(exact_limit));

        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut over_limit = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: exact_limit - 1,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        over_limit
            .set_embedding_exact_storage(embedding_bytes)
            .expect("embedding storage fits below the aggregate limit");
        over_limit
            .set_arrangement_exact_storage(boundary_bytes)
            .expect("cell boundary storage fits below the aggregate limit");
        assert!(matches!(
            over_limit.add_snapshot_exact_storage(snapshot_bytes),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit,
                    observed,
                }
            )) if limit == exact_limit - 1 && observed == exact_limit
        ));
    }

    #[test]
    fn embedding_storage_stops_before_source_polygon_clones_exceed_the_limit() {
        let (paper, pattern, topology) = three_panel_accordion();
        let canonical_faces = topology
            .faces
            .iter()
            .map(|face| LayerFace {
                face_id: face.id,
                face_key: face.key,
            })
            .collect::<Vec<_>>();
        let mut sizing_observer = NoopGlobalFlatFoldabilityObserver;
        let mut sizing_runtime = Runtime::new(
            &mut sizing_observer,
            GlobalFlatFoldabilityLimits::default(),
            zero_work(),
        );
        let source_points = pattern
            .vertices
            .iter()
            .map(|vertex| {
                point_from_binary64(vertex.position.x, vertex.position.y, &mut sizing_runtime)
                    .expect("fixture coordinate")
            })
            .collect::<Vec<_>>();
        let source_vertex_bytes =
            exact_storage_bytes_points(&source_points).expect("source exact bytes");
        let first_clone_bytes =
            exact_storage_bytes_point(&source_points[0]).expect("first clone bytes");

        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: source_vertex_bytes,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        let result =
            build_flat_embedding(&paper, &pattern, &topology, &canonical_faces, &mut runtime);
        assert!(matches!(
            result,
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit,
                    observed,
                }
            )) if limit == source_vertex_bytes
                && observed == source_vertex_bytes + first_clone_bytes
        ));
        assert_eq!(
            runtime.exact_storage.embedding_bytes, source_vertex_bytes,
            "the rejected clone is never committed to retained storage"
        );
    }

    #[test]
    fn cell_key_encoding_admits_one_boundary_copy_and_rejects_one_byte_less() {
        let boundary = vec![
            integer_point(-1, -1),
            integer_point(1, -1),
            integer_point(1, 1),
            integer_point(-1, 1),
        ];
        let boundary_bytes =
            exact_storage_bytes_points(&boundary).expect("cell boundary exact bytes");
        let canonical_structure_bytes =
            boundary.len() * std::mem::size_of::<Vec<u8>>() + std::mem::size_of::<Vec<Vec<u8>>>();
        let exact_limit = boundary_bytes * 2 + canonical_structure_bytes;
        let faces = vec![synthetic_face(0, boundary.clone(), true)];

        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut runtime = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: exact_limit,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        runtime
            .set_arrangement_exact_storage(boundary_bytes)
            .expect("retained boundary fits");
        overlap_cell_key(&boundary, &[0], &faces, &mut runtime)
            .expect("one transient canonical boundary copy fits at equality");

        let mut observer = NoopGlobalFlatFoldabilityObserver;
        let mut over_limit = Runtime::new(
            &mut observer,
            GlobalFlatFoldabilityLimits {
                max_certificate_bytes: exact_limit - 1,
                ..GlobalFlatFoldabilityLimits::default()
            },
            zero_work(),
        );
        over_limit
            .set_arrangement_exact_storage(boundary_bytes)
            .expect("retained boundary fits below the aggregate limit");
        assert!(matches!(
            overlap_cell_key(&boundary, &[0], &faces, &mut over_limit),
            Err(FacewiseAbort::Unknown(
                GlobalFlatFoldabilityUnknownReason::ResourceLimitReached {
                    resource: FlatFoldabilityResource::CertificateBytes,
                    limit,
                    observed,
                }
            )) if limit == exact_limit - 1 && observed == exact_limit
        ));
    }
}
