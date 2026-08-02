//! Capacity-bounded derivation of the sealed shared-face-pair inventory.

use ori_domain::VertexId;

use super::*;

pub(super) fn derive_exact_shared_pair_registry_v2(
    geometry: &MaterialHingeGraphGeometry,
    pair_cap: usize,
    membership_test_cap: usize,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<Vec<OrdinaryIntervalFacePairV2>, OrdinaryIntervalErrorV2> {
    checkpoint_v2(checkpoint)?;
    let faces = geometry.face_ids();
    if faces.is_empty() {
        return Err(OrdinaryIntervalErrorV2::InvalidInput);
    }
    validate_canonical_faces_v2(faces, checkpoint)?;
    let mut membership_tests = 0usize;
    validate_boundaries_v2(
        geometry,
        &mut membership_tests,
        membership_test_cap,
        checkpoint,
    )?;

    let total_pair_count = checked_pair_count_v2(faces.len())?;
    let reserve_bound = total_pair_count.min(pair_cap);
    let mut pairs = Vec::new();
    pairs
        .try_reserve_exact(reserve_bound)
        .map_err(|_| OrdinaryIntervalErrorV2::ResourceLimit)?;
    if pairs.capacity() > reserve_bound {
        return Err(OrdinaryIntervalErrorV2::ResourceLimit);
    }

    for first_position in 0..faces.len() {
        checkpoint_v2(checkpoint)?;
        for second_position in first_position + 1..faces.len() {
            checkpoint_v2(checkpoint)?;
            let pair =
                OrdinaryIntervalFacePairV2::new(faces[first_position], faces[second_position])
                    .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?;
            if pair.first != faces[first_position] || pair.second != faces[second_position] {
                return Err(OrdinaryIntervalErrorV2::InvalidInput);
            }
            let shared_vertex_count = shared_vertex_count_v2(
                geometry,
                pair,
                &mut membership_tests,
                membership_test_cap,
                checkpoint,
            )?;
            if !is_admitted_shared_vertex_count_v2(shared_vertex_count)? {
                continue;
            }
            let next_len = pairs
                .len()
                .checked_add(1)
                .filter(|value| *value <= pair_cap)
                .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
            if pairs.len() == pairs.capacity() {
                return Err(OrdinaryIntervalErrorV2::ResourceLimit);
            }
            if pairs
                .last()
                .is_some_and(|previous| compare_pair_v2(previous, &pair) != Ordering::Less)
            {
                return Err(OrdinaryIntervalErrorV2::InvalidInput);
            }
            pairs.push(pair);
            debug_assert_eq!(pairs.len(), next_len);
        }
    }
    Ok(pairs)
}

fn validate_canonical_faces_v2(
    faces: &[FaceId],
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(), OrdinaryIntervalErrorV2> {
    for pair in faces.windows(2) {
        checkpoint_v2(checkpoint)?;
        if pair[0].canonical_bytes() >= pair[1].canonical_bytes() {
            return Err(OrdinaryIntervalErrorV2::InvalidInput);
        }
    }
    Ok(())
}

fn validate_boundaries_v2(
    geometry: &MaterialHingeGraphGeometry,
    membership_tests: &mut usize,
    membership_test_cap: usize,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(), OrdinaryIntervalErrorV2> {
    for face in geometry.face_ids() {
        checkpoint_v2(checkpoint)?;
        let boundary = geometry
            .face_boundary_vertices(*face)
            .filter(|vertices| vertices.len() >= 3)
            .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?;
        for vertex in boundary {
            checkpoint_v2(checkpoint)?;
            let point = geometry
                .vertex_position(*vertex)
                .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?;
            if !point.x().is_finite()
                || !point.y().is_finite()
                || !point.z().is_finite()
                || point.y() != 0.0
            {
                return Err(OrdinaryIntervalErrorV2::InvalidInput);
            }
        }
        validate_unique_boundary_vertices_v2(
            boundary,
            membership_tests,
            membership_test_cap,
            checkpoint,
        )?;
    }
    Ok(())
}

fn validate_unique_boundary_vertices_v2(
    boundary: &[VertexId],
    membership_tests: &mut usize,
    membership_test_cap: usize,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<(), OrdinaryIntervalErrorV2> {
    for (position, vertex) in boundary.iter().enumerate() {
        checkpoint_v2(checkpoint)?;
        for other in &boundary[position + 1..] {
            checkpoint_v2(checkpoint)?;
            charge_membership_test_v2(membership_tests, membership_test_cap)?;
            if vertex == other {
                return Err(OrdinaryIntervalErrorV2::InvalidInput);
            }
        }
    }
    Ok(())
}

fn shared_vertex_count_v2(
    geometry: &MaterialHingeGraphGeometry,
    pair: OrdinaryIntervalFacePairV2,
    membership_tests: &mut usize,
    membership_test_cap: usize,
    checkpoint: &mut impl FnMut() -> Result<(), OrdinaryIntervalStopV2>,
) -> Result<usize, OrdinaryIntervalErrorV2> {
    let first = geometry
        .face_boundary_vertices(pair.first)
        .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?;
    let second = geometry
        .face_boundary_vertices(pair.second)
        .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?;
    let mut shared_vertex_count = 0usize;
    for left in first {
        checkpoint_v2(checkpoint)?;
        for right in second {
            checkpoint_v2(checkpoint)?;
            charge_membership_test_v2(membership_tests, membership_test_cap)?;
            if left == right {
                shared_vertex_count = shared_vertex_count
                    .checked_add(1)
                    .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
            }
        }
    }
    Ok(shared_vertex_count)
}

fn charge_membership_test_v2(
    membership_tests: &mut usize,
    membership_test_cap: usize,
) -> Result<(), OrdinaryIntervalErrorV2> {
    *membership_tests = membership_tests
        .checked_add(1)
        .filter(|value| *value <= membership_test_cap)
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)?;
    Ok(())
}

fn is_admitted_shared_vertex_count_v2(
    shared_vertex_count: usize,
) -> Result<bool, OrdinaryIntervalErrorV2> {
    match shared_vertex_count {
        0 => Ok(false),
        1 | 2 => Ok(true),
        _ => Err(OrdinaryIntervalErrorV2::InvalidInput),
    }
}

fn checked_pair_count_v2(face_count: usize) -> Result<usize, OrdinaryIntervalErrorV2> {
    face_count
        .checked_mul(
            face_count
                .checked_sub(1)
                .ok_or(OrdinaryIntervalErrorV2::InvalidInput)?,
        )
        .map(|value| value / 2)
        .ok_or(OrdinaryIntervalErrorV2::ResourceLimit)
}

#[cfg(test)]
mod tests {
    use ori_domain::{FaceId, VertexId};

    use super::*;

    #[test]
    fn malformed_face_order_and_boundary_duplicates_fail_closed() {
        let mut faces = [FaceId::new(), FaceId::new()];
        faces.sort_unstable_by_key(FaceId::canonical_bytes);
        let mut no_stop = || Ok(());
        assert_eq!(
            validate_canonical_faces_v2(&[faces[0], faces[0]], &mut no_stop),
            Err(OrdinaryIntervalErrorV2::InvalidInput),
        );
        assert_eq!(
            validate_canonical_faces_v2(&[faces[1], faces[0]], &mut no_stop),
            Err(OrdinaryIntervalErrorV2::InvalidInput),
        );

        let vertex = VertexId::new();
        let mut membership_tests = 0usize;
        assert_eq!(
            validate_unique_boundary_vertices_v2(
                &[vertex, vertex],
                &mut membership_tests,
                usize::MAX,
                &mut no_stop,
            ),
            Err(OrdinaryIntervalErrorV2::InvalidInput),
        );
    }

    #[test]
    fn more_than_two_shared_vertices_fails_closed() {
        assert_eq!(is_admitted_shared_vertex_count_v2(0), Ok(false));
        assert_eq!(is_admitted_shared_vertex_count_v2(1), Ok(true));
        assert_eq!(is_admitted_shared_vertex_count_v2(2), Ok(true));
        assert_eq!(
            is_admitted_shared_vertex_count_v2(3),
            Err(OrdinaryIntervalErrorV2::InvalidInput),
        );
    }
}
