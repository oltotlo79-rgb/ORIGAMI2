//! Issuer-bound projection authority for one incident triangular hinge pair.
//!
//! The lower positive-thickness pipeline is deliberately pair-local: all of
//! its fixed-size arrays contain exactly the two faces incident to one
//! authenticated hinge.  This authority keeps that pair from becoming a
//! detached two-face model.  It retains the whole native model/pose issuer,
//! the whole exact pose object, the selected face and hinge indexes, the
//! canonical excluded-face set, the full parent counts, the bit-exact angle
//! and thickness, and the native affine transform bits.
//!
//! Parent cardinality is independent of the pair, but remains bounded by the
//! established positive-thickness tree hard cap. The authority is
//! intentionally non-cloneable and non-serializable.

use ori_domain::EdgeId;
use ori_kinematics::{
    BoundMaterialTreePose, MaterialHingePairCanonicalInputV1, MaterialHingePairProjectionV1,
    RigidTransform, prepare_material_hinge_pair_projection_v1,
    revalidate_material_hinge_pair_projection_v1,
};

use super::*;

const PAIR_FACE_COUNT: usize = 2;
const MIN_PARENT_FACE_COUNT: usize = PAIR_FACE_COUNT;
const MIN_PARENT_HINGE_COUNT: usize = 1;
const HARD_MAX_PARENT_HINGE_COUNT: usize = MAX_COMPOSED_THICKNESS_HINGES_V1;
const HARD_MAX_PARENT_FACE_COUNT: usize = HARD_MAX_PARENT_HINGE_COUNT + 1;
const HARD_MAX_EXCLUDED_FACE_INDEXES: usize = HARD_MAX_PARENT_FACE_COUNT - PAIR_FACE_COUNT;
const HARD_MAX_EXCLUDED_FACE_INDEX_BINDINGS: usize =
    HARD_MAX_PARENT_HINGE_COUNT * HARD_MAX_EXCLUDED_FACE_INDEXES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProjectedPairAuthorityLimitsV1 {
    pub(super) max_parent_faces: usize,
    pub(super) max_parent_hinges: usize,
    pub(super) max_excluded_face_indexes: usize,
    pub(super) max_excluded_face_index_bindings: usize,
}

impl Default for ProjectedPairAuthorityLimitsV1 {
    fn default() -> Self {
        Self {
            max_parent_faces: HARD_MAX_PARENT_FACE_COUNT,
            max_parent_hinges: HARD_MAX_PARENT_HINGE_COUNT,
            max_excluded_face_indexes: HARD_MAX_EXCLUDED_FACE_INDEXES,
            max_excluded_face_index_bindings: HARD_MAX_EXCLUDED_FACE_INDEX_BINDINGS,
        }
    }
}

impl ProjectedPairAuthorityLimitsV1 {
    pub(super) fn clamped_to_hard(self) -> Self {
        Self {
            max_parent_faces: self.max_parent_faces.min(HARD_MAX_PARENT_FACE_COUNT),
            max_parent_hinges: self.max_parent_hinges.min(HARD_MAX_PARENT_HINGE_COUNT),
            max_excluded_face_indexes: self
                .max_excluded_face_indexes
                .min(HARD_MAX_EXCLUDED_FACE_INDEXES),
            max_excluded_face_index_bindings: self
                .max_excluded_face_index_bindings
                .min(HARD_MAX_EXCLUDED_FACE_INDEX_BINDINGS),
        }
    }
}

/// Non-cloneable authority that projects one pair without replacing its
/// whole-parent issuer.
#[derive(Debug)]
pub(super) struct ProjectedPairAuthorityV1<'exact, 'pose> {
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    projection: MaterialHingePairProjectionV1<'pose>,
    face_indexes: [usize; PAIR_FACE_COUNT],
    excluded_face_indexes: Vec<usize>,
    hinge_index: usize,
    edge: EdgeId,
    full_face_count: usize,
    full_hinge_count: usize,
    excluded_face_index_bindings: usize,
    limits: ProjectedPairAuthorityLimitsV1,
    angle_bits: u64,
    paper_thickness_bits: u64,
    exact_binary64_affine_bits: [[[u64; 4]; 3]; PAIR_FACE_COUNT],
}

/// Small consumer-side view minted only after the complete authority has been
/// revalidated.  It carries indexes, not owned geometry or policy authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RevalidatedProjectedPairAuthorityV1 {
    face_indexes: [usize; PAIR_FACE_COUNT],
    hinge_index: usize,
    edge: EdgeId,
    full_face_count: usize,
    full_hinge_count: usize,
}

impl RevalidatedProjectedPairAuthorityV1 {
    pub(super) const fn face_indexes(self) -> [usize; PAIR_FACE_COUNT] {
        self.face_indexes
    }

    pub(super) const fn hinge_index(self) -> usize {
        self.hinge_index
    }

    pub(super) const fn edge(self) -> EdgeId {
        self.edge
    }

    pub(super) const fn full_face_count(self) -> usize {
        self.full_face_count
    }

    pub(super) const fn full_hinge_count(self) -> usize {
        self.full_hinge_count
    }

    pub(super) fn face_ids(
        self,
        exact: &RationalCayleyTreePose<'_>,
    ) -> Option<[ori_domain::FaceId; PAIR_FACE_COUNT]> {
        Some([
            exact.faces.get(self.face_indexes[0])?.face,
            exact.faces.get(self.face_indexes[1])?.face,
        ])
    }
}

pub(super) fn prepare_projected_pair_authority_v1<'exact, 'pose>(
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    edge: EdgeId,
    paper_thickness_mm: f64,
) -> Option<ProjectedPairAuthorityV1<'exact, 'pose>> {
    prepare_projected_pair_authority_with_limits_v1(
        exact,
        bound,
        edge,
        paper_thickness_mm,
        ProjectedPairAuthorityLimitsV1::default(),
    )
}

pub(super) fn prepare_projected_pair_authority_with_limits_v1<'exact, 'pose>(
    exact: &'exact RationalCayleyTreePose<'pose>,
    bound: BoundMaterialTreePose<'pose>,
    edge: EdgeId,
    paper_thickness_mm: f64,
    limits: ProjectedPairAuthorityLimitsV1,
) -> Option<ProjectedPairAuthorityV1<'exact, 'pose>> {
    let limits = limits.clamped_to_hard();
    if !positive_finite_binary64(paper_thickness_mm)
        || exact.version != RATIONAL_CAYLEY_TREE_POSE_V1
        || !exact.is_for(bound)
        || !std::ptr::eq(exact.bound.model(), bound.model())
        || !std::ptr::eq(exact.bound.pose(), bound.pose())
        || bound.model().face_ids() != bound.pose().face_ids()
        || bound.model().hinges() != bound.pose().hinges()
    {
        return None;
    }

    let full_face_count = bound.model().face_ids().len();
    let full_hinge_count = bound.model().hinges().len();
    let excluded_face_index_bindings =
        projected_tree_counts_fit_limits_v1(full_face_count, full_hinge_count, limits)?;
    if exact.faces.len() != full_face_count
        || exact.hinges.len() != full_hinge_count
        || bound.pose().hinge_angles().len() != full_hinge_count
        || exact.faces.iter().any(|face| face.boundary.len() != 3)
        || bound.model().face_ids().iter().any(|face| {
            bound.face_boundary(*face).is_none_or(|boundary| {
                boundary.vertices().len() != 3 || boundary.edges().len() != 3
            })
        })
    {
        return None;
    }

    let projection = prepare_material_hinge_pair_projection_v1(bound, edge).ok()?;
    let input = revalidate_material_hinge_pair_projection_v1(&projection, bound)?;
    if !input_matches_whole_exact_parent(
        &input,
        exact,
        bound,
        full_face_count,
        full_hinge_count,
        limits,
    ) {
        return None;
    }

    Some(ProjectedPairAuthorityV1 {
        exact,
        bound,
        face_indexes: input.face_indexes,
        excluded_face_indexes: input.excluded_face_indexes.clone(),
        hinge_index: input.hinge_index,
        edge: input.edge,
        full_face_count,
        full_hinge_count,
        excluded_face_index_bindings,
        limits,
        angle_bits: input.angle_degrees.to_bits(),
        paper_thickness_bits: paper_thickness_mm.to_bits(),
        exact_binary64_affine_bits: input.exact_binary64_affine_bits,
        projection,
    })
}

pub(super) fn revalidate_projected_pair_authority_v1(
    authority: &ProjectedPairAuthorityV1<'_, '_>,
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    paper_thickness_mm: f64,
) -> Option<RevalidatedProjectedPairAuthorityV1> {
    let limits = authority.limits.clamped_to_hard();
    let excluded_face_index_bindings = projected_tree_counts_fit_limits_v1(
        authority.full_face_count,
        authority.full_hinge_count,
        limits,
    )?;
    if !positive_finite_binary64(paper_thickness_mm)
        || !std::ptr::eq(authority.exact, exact)
        || !std::ptr::eq(authority.bound.model(), bound.model())
        || !std::ptr::eq(authority.bound.pose(), bound.pose())
        || authority.paper_thickness_bits != paper_thickness_mm.to_bits()
        || authority.full_face_count != exact.faces.len()
        || authority.full_face_count != bound.model().face_ids().len()
        || authority.full_hinge_count != exact.hinges.len()
        || authority.full_hinge_count != bound.model().hinges().len()
        || authority.full_hinge_count != bound.pose().hinge_angles().len()
        || authority.excluded_face_index_bindings != excluded_face_index_bindings
        || authority.limits != limits
        || exact.version != RATIONAL_CAYLEY_TREE_POSE_V1
        || !exact.is_for(bound)
    {
        return None;
    }

    let input = revalidate_material_hinge_pair_projection_v1(&authority.projection, bound)?;
    if authority.face_indexes != input.face_indexes
        || authority.excluded_face_indexes != input.excluded_face_indexes
        || authority.hinge_index != input.hinge_index
        || authority.edge != input.edge
        || authority.angle_bits != input.angle_degrees.to_bits()
        || authority.exact_binary64_affine_bits != input.exact_binary64_affine_bits
        || !input_matches_whole_exact_parent(
            &input,
            exact,
            bound,
            authority.full_face_count,
            authority.full_hinge_count,
            limits,
        )
    {
        return None;
    }

    Some(RevalidatedProjectedPairAuthorityV1 {
        face_indexes: authority.face_indexes,
        hinge_index: authority.hinge_index,
        edge: authority.edge,
        full_face_count: authority.full_face_count,
        full_hinge_count: authority.full_hinge_count,
    })
}

pub(super) fn checked_excluded_face_index_binding_count(
    face_count: usize,
    hinge_count: usize,
) -> Option<usize> {
    let excluded_per_pair = face_count.checked_sub(PAIR_FACE_COUNT)?;
    hinge_count.checked_mul(excluded_per_pair)
}

pub(super) fn projected_tree_counts_fit_limits_v1(
    face_count: usize,
    hinge_count: usize,
    limits: ProjectedPairAuthorityLimitsV1,
) -> Option<usize> {
    let limits = limits.clamped_to_hard();
    let excluded_face_indexes = face_count.checked_sub(PAIR_FACE_COUNT)?;
    let excluded_face_index_bindings =
        checked_excluded_face_index_binding_count(face_count, hinge_count)?;
    (supported_parent_counts(face_count, hinge_count, limits)
        && excluded_face_indexes <= limits.max_excluded_face_indexes
        && excluded_face_index_bindings <= limits.max_excluded_face_index_bindings)
        .then_some(excluded_face_index_bindings)
}

fn supported_parent_counts(
    face_count: usize,
    hinge_count: usize,
    limits: ProjectedPairAuthorityLimitsV1,
) -> bool {
    face_count >= MIN_PARENT_FACE_COUNT
        && hinge_count >= MIN_PARENT_HINGE_COUNT
        && face_count <= limits.max_parent_faces
        && hinge_count <= limits.max_parent_hinges
        && hinge_count.checked_add(1) == Some(face_count)
}

fn input_matches_whole_exact_parent(
    input: &MaterialHingePairCanonicalInputV1,
    exact: &RationalCayleyTreePose<'_>,
    bound: BoundMaterialTreePose<'_>,
    full_face_count: usize,
    full_hinge_count: usize,
    limits: ProjectedPairAuthorityLimitsV1,
) -> bool {
    if !supported_parent_counts(full_face_count, full_hinge_count, limits)
        || input.face_indexes[0] == input.face_indexes[1]
        || input
            .face_indexes
            .iter()
            .any(|index| *index >= full_face_count)
        || input.hinge_index >= full_hinge_count
        || input.boundaries.iter().any(|boundary| boundary.len() != 3)
        || input
            .boundary_edges
            .iter()
            .any(|boundary| boundary.len() != 3)
        || input
            .rest_positions
            .iter()
            .any(|positions| positions.len() != 3)
        || input.excluded_face_indexes.len() > limits.max_excluded_face_indexes
        || !excluded_face_indexes_are_complete(
            &input.excluded_face_indexes,
            input.face_indexes,
            full_face_count,
        )
        || input.exact_binary64_affine_bits != input.world_transforms.map(binary64_affine_bits)
    {
        return false;
    }

    let Some(native_hinge) = bound.model().hinges().get(input.hinge_index) else {
        return false;
    };
    let Some(exact_hinge) = exact.hinges.get(input.hinge_index) else {
        return false;
    };
    let Some(native_angle) = bound.pose().hinge_angles().get(input.hinge_index) else {
        return false;
    };
    let exact_faces = input.face_indexes.map(|index| exact.faces.get(index));
    let Some(left_exact) = exact_faces[0] else {
        return false;
    };
    let Some(right_exact) = exact_faces[1] else {
        return false;
    };
    let exact_pair_matches = (exact_hinge.parent == input.faces[0]
        && exact_hinge.child == input.faces[1])
        || (exact_hinge.parent == input.faces[1] && exact_hinge.child == input.faces[0]);

    input.edge == native_hinge.edge()
        && input.edge == exact_hinge.edge
        && input.faces == [native_hinge.left_face(), native_hinge.right_face()]
        && input.face_indexes
            == [
                bound
                    .model()
                    .face_ids()
                    .iter()
                    .position(|face| *face == input.faces[0])
                    .unwrap_or(usize::MAX),
                bound
                    .model()
                    .face_ids()
                    .iter()
                    .position(|face| *face == input.faces[1])
                    .unwrap_or(usize::MAX),
            ]
        && left_exact.face == input.faces[0]
        && right_exact.face == input.faces[1]
        && exact_pair_matches
        && input.assignment == native_hinge.assignment()
        && native_angle.edge() == input.edge
        && input.angle_degrees.to_bits() == native_angle.angle_degrees().to_bits()
        && input.angle_degrees.to_bits() == exact_hinge.angle_magnitude_bits
}

fn excluded_face_indexes_are_complete(
    excluded_face_indexes: &[usize],
    selected_face_indexes: [usize; PAIR_FACE_COUNT],
    full_face_count: usize,
) -> bool {
    if selected_face_indexes[0] == selected_face_indexes[1]
        || selected_face_indexes
            .iter()
            .any(|index| *index >= full_face_count)
        || excluded_face_indexes.len() != full_face_count.saturating_sub(PAIR_FACE_COUNT)
    {
        return false;
    }
    let mut excluded_slot = 0;
    for face_index in 0..full_face_count {
        if selected_face_indexes.contains(&face_index) {
            continue;
        }
        if excluded_face_indexes.get(excluded_slot) != Some(&face_index) {
            return false;
        }
        excluded_slot += 1;
    }
    excluded_slot == excluded_face_indexes.len()
}

fn binary64_affine_bits(transform: RigidTransform) -> [[u64; 4]; 3] {
    let rows = transform.rotation_rows();
    let translation = transform.translation();
    [
        [
            rows[0][0].to_bits(),
            rows[0][1].to_bits(),
            rows[0][2].to_bits(),
            translation.x().to_bits(),
        ],
        [
            rows[1][0].to_bits(),
            rows[1][1].to_bits(),
            rows[1][2].to_bits(),
            translation.y().to_bits(),
        ],
        [
            rows[2][0].to_bits(),
            rows[2][1].to_bits(),
            rows[2][2].to_bits(),
            translation.z().to_bits(),
        ],
    ]
}

#[cfg(test)]
impl ProjectedPairAuthorityV1<'_, '_> {
    pub(super) fn flip_transform_bit_for_test(&mut self, face: usize, row: usize, column: usize) {
        self.exact_binary64_affine_bits[face][row][column] ^= 1;
    }

    pub(super) fn clear_excluded_faces_for_test(&mut self) {
        self.excluded_face_indexes.clear();
    }

    pub(super) fn remove_last_excluded_face_for_test(&mut self) {
        self.excluded_face_indexes.pop();
    }

    pub(super) fn duplicate_first_excluded_face_for_test(&mut self) {
        if let Some(first) = self.excluded_face_indexes.first().copied() {
            self.excluded_face_indexes.push(first);
        }
    }

    pub(super) fn reverse_excluded_faces_for_test(&mut self) {
        self.excluded_face_indexes.reverse();
    }

    pub(super) fn append_selected_face_to_excluded_for_test(&mut self) {
        self.excluded_face_indexes.push(self.face_indexes[0]);
    }

    pub(super) fn append_out_of_range_excluded_face_for_test(&mut self) {
        self.excluded_face_indexes.push(self.full_face_count);
    }

    pub(super) fn replace_first_face_with_excluded_for_test(&mut self) {
        self.face_indexes[0] = self
            .excluded_face_indexes
            .first()
            .copied()
            .unwrap_or(usize::MAX);
    }

    pub(super) fn swap_selected_faces_for_test(&mut self) {
        self.face_indexes.swap(0, 1);
    }

    pub(super) fn invalidate_hinge_index_for_test(&mut self) {
        self.hinge_index = usize::MAX;
    }

    pub(super) fn replace_edge_for_test(&mut self, edge: EdgeId) {
        self.edge = edge;
    }

    pub(super) fn increment_full_face_count_for_test(&mut self) {
        self.full_face_count = self.full_face_count.saturating_add(1);
    }

    pub(super) fn increment_full_hinge_count_for_test(&mut self) {
        self.full_hinge_count = self.full_hinge_count.saturating_add(1);
    }

    pub(super) fn increment_excluded_binding_work_for_test(&mut self) {
        self.excluded_face_index_bindings = self.excluded_face_index_bindings.saturating_add(1);
    }

    pub(super) fn increment_angle_bits_for_test(&mut self) {
        self.angle_bits = self.angle_bits.wrapping_add(1);
    }
}
