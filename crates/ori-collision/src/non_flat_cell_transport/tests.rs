use std::{
    sync::{
        Arc, Barrier,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use super::*;
use ori_domain::ProjectId;

#[test]
fn work_preflight_is_inclusive_and_fail_closed() {
    assert_eq!(
        NonFlatCellTransportLimitsV1::default(),
        NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1
    );
    let limits = NonFlatCellTransportLimitsV1 {
        max_faces: 16,
        max_cells: 32,
        max_pairs: 32,
        max_boundary_points: 128,
    };
    assert_eq!(
        preflight_non_flat_cell_transport_v1(16, 32, 32, 128, limits),
        Ok(())
    );
    assert_eq!(
        preflight_non_flat_cell_transport_v1(16, 32, 31, 128, limits),
        Ok(()),
        "multiple overlap cells may share one directed pair-order entry"
    );
    for rejected in [
        (0, 0, 0, 0),
        (17, 32, 32, 128),
        (16, 33, 33, 128),
        (16, 31, 32, 128),
        (16, 32, 32, 129),
    ] {
        assert_eq!(
            preflight_non_flat_cell_transport_v1(
                rejected.0, rejected.1, rejected.2, rejected.3, limits
            ),
            Err(NonFlatCellTransportErrorV1::ResourceLimit)
        );
    }
    for expanded in [
        NonFlatCellTransportLimitsV1 {
            max_faces: usize::MAX,
            ..NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1
        },
        NonFlatCellTransportLimitsV1 {
            max_cells: usize::MAX,
            ..NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1
        },
        NonFlatCellTransportLimitsV1 {
            max_pairs: usize::MAX,
            ..NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1
        },
        NonFlatCellTransportLimitsV1 {
            max_boundary_points: usize::MAX,
            ..NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1
        },
    ] {
        assert_eq!(
            preflight_non_flat_cell_transport_v1(1, 0, 0, 0, expanded),
            Err(NonFlatCellTransportErrorV1::ResourceLimit),
            "caller configuration cannot expand any process-wide hard cap"
        );
    }
}

/// Owned adversarial fixture. It never builds a public
/// `StackedFoldNonFlatLayerOrderV1`; it only implements the generic
/// structural trait so that one condition at a time can be broken.
#[derive(Clone, Debug)]
struct StructuralFixture {
    material_faces: Vec<FaceId>,
    folded: Vec<(FaceId, u8, ExactAffineTransform)>,
    cells: Vec<(Vec<Point2>, Vec<ExactPointValue>, FaceId, FaceId)>,
    pairs: Vec<(FaceId, FaceId)>,
    cancel_after_first_material_face: Option<Arc<AtomicBool>>,
    cancel_after_first_pair_order: Option<Arc<AtomicBool>>,
    cancel_after_first_overlap_cell: Option<Arc<AtomicBool>>,
    count_accessor_calls: Option<Arc<AtomicUsize>>,
    escalating_overlap_cell_count_reads: Option<Arc<AtomicUsize>>,
    overlap_cell_access_count: Option<Arc<AtomicUsize>>,
    overlap_cell_count_override: Option<usize>,
    wait_until_first_material_face: Option<(Instant, Arc<AtomicBool>)>,
    wait_until_first_overlap_cell: Option<(Instant, Arc<AtomicBool>)>,
}

impl PartialEq for StructuralFixture {
    fn eq(&self, other: &Self) -> bool {
        self.material_faces == other.material_faces
            && self.folded == other.folded
            && self.cells == other.cells
            && self.pairs == other.pairs
    }
}

impl NonFlatLayerOrderStructuralSourceV1 for StructuralFixture {
    fn material_face_count(&self) -> usize {
        if let Some(accesses) = &self.count_accessor_calls {
            accesses.fetch_add(1, Ordering::AcqRel);
        }
        self.material_faces.len()
    }
    fn material_face_id(&self, index: usize) -> Option<FaceId> {
        let face = self.material_faces.get(index).copied();
        if index == 0 {
            if let Some(cancel) = &self.cancel_after_first_material_face {
                cancel.store(true, Ordering::Release);
            }
            if let Some((deadline, started)) = &self.wait_until_first_material_face {
                started.store(true, Ordering::Release);
                while Instant::now() < *deadline {
                    std::hint::spin_loop();
                }
            }
        }
        face
    }
    fn folded_face_count(&self) -> usize {
        if let Some(accesses) = &self.count_accessor_calls {
            accesses.fetch_add(1, Ordering::AcqRel);
        }
        self.folded.len()
    }
    fn folded_face(&self, index: usize) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>> {
        self.folded.get(index).map(
            |(face_id, axis, transform)| NonFlatFoldedFaceStructuralRefV1 {
                face_id: *face_id,
                dropped_world_axis: *axis,
                source_to_plane: transform,
            },
        )
    }
    fn overlap_cell_count(&self) -> usize {
        if let Some(accesses) = &self.count_accessor_calls {
            accesses.fetch_add(1, Ordering::AcqRel);
        }
        if let Some(reads) = &self.escalating_overlap_cell_count_reads {
            if reads.fetch_add(1, Ordering::AcqRel) > 0 {
                return usize::MAX;
            }
        }
        self.overlap_cell_count_override.unwrap_or(self.cells.len())
    }
    fn overlap_cell(&self, index: usize) -> Option<NonFlatOverlapCellStructuralRefV1<'_>> {
        let cell = self
            .cells
            .get(index)
            .map(
                |(boundary, exact, lower, upper)| NonFlatOverlapCellStructuralRefV1 {
                    boundary,
                    exact_boundary: exact,
                    lower_face: *lower,
                    upper_face: *upper,
                },
            );
        if let Some(accesses) = &self.overlap_cell_access_count {
            accesses.fetch_add(1, Ordering::AcqRel);
        }
        if index == 0 {
            if let Some(cancel) = &self.cancel_after_first_overlap_cell {
                cancel.store(true, Ordering::Release);
            }
            if let Some((deadline, started)) = &self.wait_until_first_overlap_cell {
                started.store(true, Ordering::Release);
                while Instant::now() < *deadline {
                    std::hint::spin_loop();
                }
            }
        }
        cell
    }
    fn face_pair_order_count(&self) -> usize {
        if let Some(accesses) = &self.count_accessor_calls {
            accesses.fetch_add(1, Ordering::AcqRel);
        }
        self.pairs.len()
    }
    fn face_pair_order(&self, index: usize) -> Option<NonFlatFacePairOrderStructuralV1> {
        let pair = self
            .pairs
            .get(index)
            .map(|(lower, upper)| NonFlatFacePairOrderStructuralV1 {
                lower_face: *lower,
                upper_face: *upper,
            });
        if index == 0 {
            if let Some(cancel) = &self.cancel_after_first_pair_order {
                cancel.store(true, Ordering::Release);
            }
        }
        pair
    }
}

struct DeclaredCapacityOverflowFixture;

impl NonFlatLayerOrderStructuralSourceV1 for DeclaredCapacityOverflowFixture {
    fn material_face_count(&self) -> usize {
        usize::MAX
    }
    fn material_face_id(&self, _index: usize) -> Option<FaceId> {
        None
    }
    fn folded_face_count(&self) -> usize {
        0
    }
    fn folded_face(&self, _index: usize) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>> {
        None
    }
    fn overlap_cell_count(&self) -> usize {
        0
    }
    fn overlap_cell(&self, _index: usize) -> Option<NonFlatOverlapCellStructuralRefV1<'_>> {
        None
    }
    fn face_pair_order_count(&self) -> usize {
        0
    }
    fn face_pair_order(&self, _index: usize) -> Option<NonFlatFacePairOrderStructuralV1> {
        None
    }
}

fn exact_integer(value: i8) -> ori_foldability::ExactRationalValue {
    ori_foldability::ExactRationalValue {
        sign: match value.cmp(&0) {
            std::cmp::Ordering::Less => ori_foldability::ExactSign::Negative,
            std::cmp::Ordering::Equal => ori_foldability::ExactSign::Zero,
            std::cmp::Ordering::Greater => ori_foldability::ExactSign::Positive,
        },
        numerator_magnitude_be: if value == 0 {
            Vec::new()
        } else {
            vec![value.unsigned_abs()]
        },
        denominator_be: vec![1],
    }
}

fn identity_transform() -> ExactAffineTransform {
    ExactAffineTransform {
        m00: exact_integer(1),
        m01: exact_integer(0),
        m10: exact_integer(0),
        m11: exact_integer(1),
        tx: exact_integer(0),
        ty: exact_integer(0),
    }
}

fn triangle_at(x_offset: i8) -> (Vec<Point2>, Vec<ExactPointValue>) {
    let points = [(x_offset, 0_i8), (x_offset + 1, 0), (x_offset, 1)];
    (
        points
            .iter()
            .map(|(x, y)| Point2::new(f64::from(*x), f64::from(*y)))
            .collect(),
        points
            .iter()
            .map(|(x, y)| ExactPointValue {
                x: exact_integer(*x),
                y: exact_integer(*y),
            })
            .collect(),
    )
}

fn triangle() -> (Vec<Point2>, Vec<ExactPointValue>) {
    triangle_at(0)
}

fn base_fixture() -> (StructuralFixture, FaceId, FaceId) {
    let a = FaceId::new();
    let b = FaceId::new();
    let (boundary, exact) = triangle();
    (
        StructuralFixture {
            material_faces: vec![a, b],
            folded: vec![(a, 0, identity_transform()), (b, 2, identity_transform())],
            cells: vec![(boundary, exact, a, b)],
            pairs: vec![(a, b)],
            cancel_after_first_material_face: None,
            cancel_after_first_pair_order: None,
            cancel_after_first_overlap_cell: None,
            count_accessor_calls: None,
            escalating_overlap_cell_count_reads: None,
            overlap_cell_access_count: None,
            overlap_cell_count_override: None,
            wait_until_first_material_face: None,
            wait_until_first_overlap_cell: None,
        },
        a,
        b,
    )
}

#[derive(Debug)]
struct AuthenticatedFixture {
    structural: StructuralFixture,
    namespace: ProjectId,
    revision: u64,
    authenticated_source_cells: usize,
    target_model_tag: u8,
    clone_count: Option<Arc<AtomicUsize>>,
    cancel_on_clone: Option<Arc<AtomicBool>>,
}

impl Clone for AuthenticatedFixture {
    fn clone(&self) -> Self {
        if let Some(clones) = &self.clone_count {
            clones.fetch_add(1, Ordering::AcqRel);
        }
        if let Some(cancel) = &self.cancel_on_clone {
            cancel.store(true, Ordering::Release);
        }
        Self {
            structural: self.structural.clone(),
            namespace: self.namespace,
            revision: self.revision,
            authenticated_source_cells: self.authenticated_source_cells,
            target_model_tag: self.target_model_tag,
            clone_count: self.clone_count.clone(),
            cancel_on_clone: self.cancel_on_clone.clone(),
        }
    }
}

impl PartialEq for AuthenticatedFixture {
    fn eq(&self, other: &Self) -> bool {
        self.structural == other.structural
            && self.namespace == other.namespace
            && self.revision == other.revision
            && self.authenticated_source_cells == other.authenticated_source_cells
            && self.target_model_tag == other.target_model_tag
    }
}

impl NonFlatLayerOrderStructuralSourceV1 for AuthenticatedFixture {
    fn material_face_count(&self) -> usize {
        self.structural.material_face_count()
    }
    fn material_face_id(&self, index: usize) -> Option<FaceId> {
        self.structural.material_face_id(index)
    }
    fn folded_face_count(&self) -> usize {
        self.structural.folded_face_count()
    }
    fn folded_face(&self, index: usize) -> Option<NonFlatFoldedFaceStructuralRefV1<'_>> {
        self.structural.folded_face(index)
    }
    fn overlap_cell_count(&self) -> usize {
        self.structural.overlap_cell_count()
    }
    fn overlap_cell(&self, index: usize) -> Option<NonFlatOverlapCellStructuralRefV1<'_>> {
        self.structural.overlap_cell(index)
    }
    fn face_pair_order_count(&self) -> usize {
        self.structural.face_pair_order_count()
    }
    fn face_pair_order(&self, index: usize) -> Option<NonFlatFacePairOrderStructuralV1> {
        self.structural.face_pair_order(index)
    }
}

impl NonFlatLayerOrderTransportSourceV1 for AuthenticatedFixture {
    fn identity_namespace_v1(&self) -> ProjectId {
        self.namespace
    }
    fn target_revision_v1(&self) -> u64 {
        self.revision
    }
    fn source_overlap_cells_authenticated_v1(&self) -> usize {
        self.authenticated_source_cells
    }
    fn same_target_model_v1(&self, other: &Self) -> bool {
        self.target_model_tag == other.target_model_tag
    }
}

fn authenticated_transition(
    structural: StructuralFixture,
) -> (AuthenticatedFixture, AuthenticatedFixture) {
    let namespace = ProjectId::new();
    let source = AuthenticatedFixture {
        structural: structural.clone(),
        namespace,
        revision: 9,
        authenticated_source_cells: 0,
        target_model_tag: 1,
        clone_count: None,
        cancel_on_clone: None,
    };
    let target = AuthenticatedFixture {
        structural,
        namespace,
        revision: 10,
        authenticated_source_cells: source.overlap_cell_count(),
        target_model_tag: 2,
        clone_count: None,
        cancel_on_clone: None,
    };
    (source, target)
}

#[test]
fn a_generic_source_can_mint_only_its_own_typed_bounded_proof() {
    let (structural, _, _) = base_fixture();
    let namespace = ProjectId::new();
    let source = AuthenticatedFixture {
        structural: structural.clone(),
        namespace,
        revision: 9,
        authenticated_source_cells: 0,
        target_model_tag: 1,
        clone_count: None,
        cancel_on_clone: None,
    };
    let target = AuthenticatedFixture {
        structural,
        namespace,
        revision: 10,
        authenticated_source_cells: source.overlap_cell_count(),
        target_model_tag: 2,
        clone_count: None,
        cancel_on_clone: None,
    };
    let proof: NonFlatCellTransportProofV1<AuthenticatedFixture> =
        certify_non_flat_cell_transport_v1(&source, &target)
            .expect("the exact generic fixture is admissible");
    let controlled = certify_non_flat_cell_transport_with_control_v1(
        &source,
        &target,
        NonFlatCellTransportLimitsV1::default(),
        &CooperativeOperationControlV1::unbounded(),
    )
    .expect("the unbounded controlled path makes the same decision");
    assert_eq!(controlled, proof);
    assert!(proof.is_for(&source, &target));
    assert_eq!(proof.target(), &target);
    assert_eq!(
        certify_non_flat_cell_transport_with_limits_v1(
            &source,
            &target,
            NonFlatCellTransportLimitsV1 {
                max_faces: 1,
                ..NonFlatCellTransportLimitsV1::default()
            },
        ),
        Err(NonFlatCellTransportErrorV1::ResourceLimit)
    );
}

#[test]
fn bounded_transport_certification_accepts_multiple_cells_for_one_pair() {
    let (mut structural, a, b) = base_fixture();
    let (boundary, exact) = triangle_at(2);
    structural.cells.push((boundary, exact, a, b));
    structural.pairs.push((a, b));
    let namespace = ProjectId::new();
    let source = AuthenticatedFixture {
        structural: structural.clone(),
        namespace,
        revision: 9,
        authenticated_source_cells: 0,
        target_model_tag: 1,
        clone_count: None,
        cancel_on_clone: None,
    };
    let target = AuthenticatedFixture {
        structural,
        namespace,
        revision: 10,
        authenticated_source_cells: source.overlap_cell_count(),
        target_model_tag: 2,
        clone_count: None,
        cancel_on_clone: None,
    };

    let proof = certify_non_flat_cell_transport_v1(&source, &target)
        .expect("cell-aligned repeated orders cover both overlap components");
    assert!(proof.is_for(&source, &target));
}

#[test]
fn zero_overlap_cells_and_zero_pair_orders_remain_certifiable() {
    let (mut structural, _, _) = base_fixture();
    structural.cells.clear();
    structural.pairs.clear();
    let (source, target) = authenticated_transition(structural);
    assert_eq!(source.overlap_cell_count(), 0);
    assert_eq!(target.source_overlap_cells_authenticated_v1(), 0);

    let proof = certify_non_flat_cell_transport_v1(&source, &target)
        .expect("a complete non-overlapping layer order has no supporting cells");
    assert!(proof.is_for(&source, &target));
}

#[test]
fn certification_rejects_count_limit_before_reading_any_target_cell() {
    let (structural, _, _) = base_fixture();
    let (source, mut target) = authenticated_transition(structural);
    let accesses = Arc::new(AtomicUsize::new(0));
    target.structural.overlap_cell_access_count = Some(Arc::clone(&accesses));

    assert_eq!(
        certify_non_flat_cell_transport_with_control_v1(
            &source,
            &target,
            NonFlatCellTransportLimitsV1 {
                max_cells: 0,
                ..NonFlatCellTransportLimitsV1::default()
            },
            &CooperativeOperationControlV1::unbounded(),
        ),
        Err(NonFlatCellTransportErrorV1::ResourceLimit)
    );
    assert_eq!(
        accesses.load(Ordering::Acquire),
        0,
        "count-only preflight must precede every overlap-cell accessor"
    );
}

#[test]
fn expanded_configured_limits_are_rejected_before_any_trait_accessor() {
    let (structural, _, _) = base_fixture();
    let (mut source, mut target) = authenticated_transition(structural);
    let count_accesses = Arc::new(AtomicUsize::new(0));
    let cell_accesses = Arc::new(AtomicUsize::new(0));
    source.structural.count_accessor_calls = Some(Arc::clone(&count_accesses));
    target.structural.count_accessor_calls = Some(Arc::clone(&count_accesses));
    source.structural.overlap_cell_access_count = Some(Arc::clone(&cell_accesses));
    target.structural.overlap_cell_access_count = Some(Arc::clone(&cell_accesses));

    assert_eq!(
        certify_non_flat_cell_transport_with_control_v1(
            &source,
            &target,
            NonFlatCellTransportLimitsV1 {
                max_faces: usize::MAX,
                max_cells: usize::MAX,
                max_pairs: usize::MAX,
                max_boundary_points: usize::MAX,
            },
            &CooperativeOperationControlV1::unbounded(),
        ),
        Err(NonFlatCellTransportErrorV1::ResourceLimit)
    );
    assert_eq!(count_accesses.load(Ordering::Acquire), 0);
    assert_eq!(cell_accesses.load(Ordering::Acquire), 0);
}

#[test]
fn oversized_source_is_rejected_before_cell_access_or_clone() {
    let (structural, _, _) = base_fixture();
    let (mut source, target) = authenticated_transition(structural);
    source.structural.overlap_cell_count_override = Some(
        NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1
            .max_cells
            .checked_add(1)
            .expect("the hard cell cap has a successor"),
    );
    let cell_accesses = Arc::new(AtomicUsize::new(0));
    source.structural.overlap_cell_access_count = Some(Arc::clone(&cell_accesses));
    let clones = Arc::new(AtomicUsize::new(0));
    source.clone_count = Some(Arc::clone(&clones));

    assert_eq!(
        certify_non_flat_cell_transport_with_control_v1(
            &source,
            &target,
            NonFlatCellTransportLimitsV1::default(),
            &CooperativeOperationControlV1::unbounded(),
        ),
        Err(NonFlatCellTransportErrorV1::ResourceLimit)
    );
    assert_eq!(cell_accesses.load(Ordering::Acquire), 0);
    assert_eq!(clones.load(Ordering::Acquire), 0);
}

#[test]
fn stateful_target_count_escalation_fails_before_structural_reserve_or_clone() {
    let (structural, _, _) = base_fixture();
    let (mut source, mut target) = authenticated_transition(structural);
    let overlap_count_reads = Arc::new(AtomicUsize::new(0));
    target.structural.escalating_overlap_cell_count_reads = Some(Arc::clone(&overlap_count_reads));
    let clones = Arc::new(AtomicUsize::new(0));
    source.clone_count = Some(Arc::clone(&clones));
    target.clone_count = Some(Arc::clone(&clones));

    assert_eq!(
        certify_non_flat_cell_transport_with_control_v1(
            &source,
            &target,
            NonFlatCellTransportLimitsV1::default(),
            &CooperativeOperationControlV1::unbounded(),
        ),
        Err(NonFlatCellTransportErrorV1::BindingMismatch)
    );
    assert_eq!(
        overlap_count_reads.load(Ordering::Acquire),
        2,
        "the captured count is reread exactly once before structural allocation"
    );
    assert_eq!(clones.load(Ordering::Acquire), 0);
}

#[test]
fn controlled_certification_cancels_mid_boundary_without_cloning_a_proof() {
    let (mut structural, a, b) = base_fixture();
    let (boundary, exact) = triangle_at(2);
    structural.cells.push((boundary, exact, a, b));
    structural.pairs.push((a, b));
    let (mut source, mut target) = authenticated_transition(structural);
    let cancelled = Arc::new(AtomicBool::new(false));
    target.structural.cancel_after_first_overlap_cell = Some(Arc::clone(&cancelled));
    let clones = Arc::new(AtomicUsize::new(0));
    source.clone_count = Some(Arc::clone(&clones));
    target.clone_count = Some(Arc::clone(&clones));

    assert_eq!(
        certify_non_flat_cell_transport_with_control_v1(
            &source,
            &target,
            NonFlatCellTransportLimitsV1::default(),
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(NonFlatCellTransportErrorV1::Cancelled)
    );
    assert_eq!(
        clones.load(Ordering::Acquire),
        0,
        "a stopped certification must not retain source or target into a proof"
    );
}

#[test]
fn controlled_certification_observes_deadline_after_one_boundary_access() {
    let (structural, _, _) = base_fixture();
    let (mut source, mut target) = authenticated_transition(structural);
    let entered = Arc::new(AtomicBool::new(false));
    let clones = Arc::new(AtomicUsize::new(0));
    source.clone_count = Some(Arc::clone(&clones));
    target.clone_count = Some(Arc::clone(&clones));
    let start_gate = Arc::new(Barrier::new(2));

    let result = std::thread::scope(|scope| {
        let worker_gate = Arc::clone(&start_gate);
        let worker_entered = Arc::clone(&entered);
        let worker = scope.spawn(move || {
            worker_gate.wait();
            let deadline = Instant::now() + Duration::from_millis(100);
            source.structural.wait_until_first_overlap_cell =
                Some((deadline, Arc::clone(&worker_entered)));
            certify_non_flat_cell_transport_with_control_v1(
                &source,
                &target,
                NonFlatCellTransportLimitsV1::default(),
                &CooperativeOperationControlV1::new(None, deadline),
            )
        });
        start_gate.wait();
        worker.join().expect("deadline worker must not panic")
    });

    assert_eq!(result, Err(NonFlatCellTransportErrorV1::DeadlineExceeded));
    assert!(entered.load(Ordering::Acquire));
    assert_eq!(
        clones.load(Ordering::Acquire),
        0,
        "deadline expiry must return before proof retention"
    );
}

#[test]
fn a_stop_during_retention_never_issues_a_proof() {
    let (structural, _, _) = base_fixture();
    let (mut source, mut target) = authenticated_transition(structural);
    let cancelled = Arc::new(AtomicBool::new(false));
    let clones = Arc::new(AtomicUsize::new(0));
    source.clone_count = Some(Arc::clone(&clones));
    target.clone_count = Some(Arc::clone(&clones));
    target.cancel_on_clone = Some(Arc::clone(&cancelled));

    assert_eq!(
        certify_non_flat_cell_transport_with_control_v1(
            &source,
            &target,
            NonFlatCellTransportLimitsV1::default(),
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(NonFlatCellTransportErrorV1::Cancelled)
    );
    assert_eq!(
        clones.load(Ordering::Acquire),
        2,
        "the final checkpoint must reject retained values before returning proof authority"
    );
}

#[test]
fn the_structural_fixture_baseline_is_complete() {
    let (fixture, _, _) = base_fixture();
    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Ok(())
    );
}

#[test]
fn controlled_structural_validation_distinguishes_stops_and_preserves_unbounded_result() {
    let (fixture, _, _) = base_fixture();
    assert_eq!(
        validate_non_flat_layer_order_structure_with_control_v1(
            &fixture,
            &CooperativeOperationControlV1::unbounded(),
        ),
        validate_non_flat_layer_order_structure_v1(&fixture)
    );

    let cancelled = AtomicBool::new(true);
    assert_eq!(
        validate_non_flat_layer_order_structure_with_control_v1(
            &fixture,
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(NonFlatCellTransportErrorV1::Cancelled)
    );
    assert_eq!(
        validate_non_flat_layer_order_structure_with_control_v1(
            &fixture,
            &CooperativeOperationControlV1::new(None, Instant::now()),
        ),
        Err(NonFlatCellTransportErrorV1::DeadlineExceeded)
    );
}

#[test]
fn controlled_structural_validation_stops_within_the_material_face_loop() {
    let (mut fixture, _, _) = base_fixture();
    let cancelled = Arc::new(AtomicBool::new(false));
    fixture.cancel_after_first_material_face = Some(Arc::clone(&cancelled));
    assert_eq!(
        validate_non_flat_layer_order_structure_with_control_v1(
            &fixture,
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(NonFlatCellTransportErrorV1::Cancelled)
    );
}

#[test]
fn controlled_structural_validation_stops_within_repeated_pair_normalization() {
    let (mut fixture, a, b) = base_fixture();
    let (boundary, exact) = triangle_at(2);
    fixture.cells.push((boundary, exact, a, b));
    fixture.pairs.push((a, b));
    let cancelled = Arc::new(AtomicBool::new(false));
    fixture.cancel_after_first_pair_order = Some(Arc::clone(&cancelled));

    assert_eq!(
        validate_non_flat_layer_order_structure_with_control_v1(
            &fixture,
            &CooperativeOperationControlV1::new(
                Some(&cancelled),
                Instant::now() + Duration::from_secs(1),
            ),
        ),
        Err(NonFlatCellTransportErrorV1::Cancelled)
    );
}

#[test]
fn controlled_structural_validation_observes_a_deadline_within_the_material_face_loop() {
    let (mut fixture, _, _) = base_fixture();
    let deadline = Instant::now() + Duration::from_millis(2);
    let entered_loop = Arc::new(AtomicBool::new(false));
    fixture.wait_until_first_material_face = Some((deadline, Arc::clone(&entered_loop)));
    assert_eq!(
        validate_non_flat_layer_order_structure_with_control_v1(
            &fixture,
            &CooperativeOperationControlV1::new(None, deadline),
        ),
        Err(NonFlatCellTransportErrorV1::DeadlineExceeded)
    );
    assert!(entered_loop.load(Ordering::Acquire));
}

#[test]
fn a_missing_folded_face_is_incomplete() {
    let (mut fixture, _, _) = base_fixture();
    fixture.folded.pop();
    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
    );
}

#[test]
fn a_duplicate_material_face_is_incomplete() {
    let (mut fixture, a, _) = base_fixture();
    fixture.material_faces = vec![a, a];
    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
    );
}

#[test]
fn structural_validation_declared_capacity_overflow_is_resource_limit() {
    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&DeclaredCapacityOverflowFixture,),
        Err(NonFlatCellTransportErrorV1::ResourceLimit)
    );
}

#[test]
fn public_structural_validation_rejects_huge_cell_count_before_cell_access() {
    let (mut fixture, _, _) = base_fixture();
    fixture.overlap_cell_count_override = Some(
        NON_FLAT_CELL_TRANSPORT_HARD_LIMITS_V1
            .max_cells
            .checked_add(1)
            .expect("the hard cell cap has a successor"),
    );
    let accesses = Arc::new(AtomicUsize::new(0));
    fixture.overlap_cell_access_count = Some(Arc::clone(&accesses));

    assert_eq!(
        validate_non_flat_layer_order_structure_v1(&fixture),
        Err(NonFlatCellTransportErrorV1::ResourceLimit)
    );
    assert_eq!(accesses.load(Ordering::Acquire), 0);
}

#[test]
fn an_out_of_range_dropped_world_axis_is_incomplete() {
    let (mut fixture, _, _) = base_fixture();
    fixture.folded[0].1 = 3;
    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
    );
}

#[test]
fn negative_zero_rounded_provenance_is_incomplete() {
    let (mut fixture, _, _) = base_fixture();
    // -0.0 and +0.0 compare equal numerically, so only the to_bits()
    // comparison rejects this rounded/exact provenance mismatch.
    fixture.cells[0].0[0] = Point2::new(-0.0, 0.0);
    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
    );
}

#[test]
fn a_pair_that_disagrees_with_its_cell_is_incomplete() {
    let (mut fixture, a, _) = base_fixture();
    let c = FaceId::new();
    fixture.pairs[0] = (a, c);
    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
    );
}

#[test]
fn an_unknown_face_in_a_cell_is_incomplete() {
    let (mut fixture, a, _) = base_fixture();
    let c = FaceId::new();
    fixture.cells[0].3 = c;
    fixture.pairs[0] = (a, c);
    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
    );
}

#[test]
fn a_self_paired_cell_is_incomplete() {
    let (mut fixture, a, _) = base_fixture();
    fixture.cells[0].3 = a;
    fixture.pairs[0] = (a, a);
    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
    );
}

#[test]
fn an_opposite_direction_cell_crosses() {
    let (mut fixture, a, b) = base_fixture();
    let (boundary, exact) = triangle();
    fixture.cells.push((boundary, exact, b, a));
    fixture.pairs.push((b, a));
    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Err(NonFlatCellTransportErrorV1::Crossing)
    );
}

#[test]
fn a_cell_and_pair_count_mismatch_is_incomplete() {
    let (mut fixture, _, _) = base_fixture();
    fixture.pairs.clear();
    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
    );
}

#[test]
fn multiple_overlap_components_can_share_one_directed_pair_order() {
    let (mut fixture, a, b) = base_fixture();
    let (boundary, exact) = triangle_at(2);
    fixture.cells.push((boundary, exact, a, b));

    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Ok(()),
        "a unique pair-order registry entry covers every component of that face pair"
    );
}

#[test]
fn branching_face_order_relations_are_complete() {
    let (mut fixture, a, b) = base_fixture();
    let c = FaceId::new();
    let d = FaceId::new();
    fixture.material_faces.extend([c, d]);
    fixture
        .folded
        .extend([(c, 1, identity_transform()), (d, 2, identity_transform())]);
    for (offset, lower, upper) in [(2, a, c), (4, b, d), (6, c, d)] {
        let (boundary, exact) = triangle_at(offset);
        fixture.cells.push((boundary, exact, lower, upper));
        fixture.pairs.push((lower, upper));
    }

    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Ok(()),
        "A→B, A→C, B→D, C→D is a valid branching set of local orders"
    );
}

#[test]
fn disconnected_face_order_components_are_complete() {
    let (mut fixture, _, _) = base_fixture();
    let c = FaceId::new();
    let d = FaceId::new();
    fixture.material_faces.extend([c, d]);
    fixture
        .folded
        .extend([(c, 1, identity_transform()), (d, 2, identity_transform())]);
    let (boundary, exact) = triangle_at(2);
    fixture.cells.push((boundary, exact, c, d));
    fixture.pairs.push((c, d));

    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Ok(())
    );
}

#[test]
fn local_orders_in_disjoint_cells_may_form_a_global_cycle() {
    let (mut fixture, a, b) = base_fixture();
    let c = FaceId::new();
    fixture.material_faces.push(c);
    fixture.folded.push((c, 1, identity_transform()));
    for (offset, lower, upper) in [(2, b, c), (4, c, a)] {
        let (boundary, exact) = triangle_at(offset);
        fixture.cells.push((boundary, exact, lower, upper));
        fixture.pairs.push((lower, upper));
    }

    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Ok(()),
        "cell-local authority must not be rejected solely by a derived global cycle"
    );
}

#[test]
fn pair_order_registry_is_independent_of_overlap_cell_iteration_order() {
    let (mut fixture, a, b) = base_fixture();
    let c = FaceId::new();
    fixture.material_faces.push(c);
    fixture.folded.push((c, 1, identity_transform()));
    let (boundary, exact) = triangle_at(2);
    fixture.cells.push((boundary, exact, b, c));
    fixture.pairs = vec![(b, c), (a, b)];

    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Ok(())
    );
}

#[test]
fn every_declared_pair_order_must_cover_at_least_one_cell() {
    let (mut fixture, a, b) = base_fixture();
    let c = FaceId::new();
    fixture.material_faces.push(c);
    fixture.folded.push((c, 1, identity_transform()));
    let (boundary, exact) = triangle_at(2);
    fixture.cells.push((boundary, exact, a, b));
    fixture.pairs.push((b, c));

    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
    );
}

#[test]
fn every_cell_must_reference_a_declared_pair_order() {
    let (mut fixture, a, b) = base_fixture();
    let c = FaceId::new();
    fixture.material_faces.push(c);
    fixture.folded.push((c, 1, identity_transform()));
    let (boundary, exact) = triangle_at(2);
    fixture.cells.push((boundary, exact, a, c));
    fixture.pairs = vec![(a, b)];

    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Err(NonFlatCellTransportErrorV1::IncompleteCoverage)
    );
}

#[test]
fn cell_aligned_repeated_same_direction_pair_orders_are_idempotent() {
    let (mut fixture, a, b) = base_fixture();
    let (boundary, exact) = triangle_at(2);
    fixture.cells.push((boundary, exact, a, b));
    fixture.pairs.push((a, b));

    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Ok(()),
        "current producers may repeat one order beside each supporting component"
    );
}

#[test]
fn a_cell_opposite_to_its_declared_pair_order_crosses() {
    let (mut fixture, a, b) = base_fixture();
    fixture.cells[0].2 = b;
    fixture.cells[0].3 = a;

    assert_eq!(
        validate_non_flat_layer_order_structural_source_v1(&fixture),
        Err(NonFlatCellTransportErrorV1::Crossing)
    );
}

#[test]
fn the_public_structure_validator_grants_no_transport_authority() {
    // Structural validation alone must never behave like certification:
    // it takes one value, returns no proof, and cannot compare revisions.
    let accepts: fn(&StructuralFixture) -> Result<(), NonFlatCellTransportErrorV1> =
        validate_non_flat_layer_order_structure_v1;
    let _ = accepts;
}
