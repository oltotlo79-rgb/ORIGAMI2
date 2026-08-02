//! Exact recognizer for the narrow ordinary affine parallel-cut theorem.
//!
//! The recognizer is deliberately independent of the adaptive interval
//! fallback. It has its own logical-work and transient-workspace ceilings and
//! returns both charges even when the schedule or geometry is not applicable.

use std::{collections::VecDeque, mem::size_of};

use ori_domain::{EdgeId, FaceId};
use ori_topology::FoldAssignment;

use super::{IntervalAttemptErrorV2, map_checkpoint_v2};
use crate::schedule::ExactParallelCutProfileErrorV2;
use crate::{CanonicalCycleScheduleV1, DyadicIntervalClosureStopV1, MaterialHingeGraphGeometry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExactParallelCutRecognitionV2 {
    NotApplicable {
        charged_work: usize,
        workspace_bytes: usize,
    },
    Proven {
        charged_work: usize,
        workspace_bytes: usize,
    },
}

fn charge_work_v2(
    work: &mut usize,
    amount: usize,
    max_work: usize,
) -> Result<(), IntervalAttemptErrorV2> {
    *work = work
        .checked_add(amount)
        .filter(|value| *value <= max_work)
        .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
    Ok(())
}

fn face_index_with_checkpoint_v2(
    faces: &[FaceId],
    face: FaceId,
    work: &mut usize,
    max_work: usize,
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<usize, IntervalAttemptErrorV2> {
    let key = face.canonical_bytes();
    let mut left = 0usize;
    let mut right = faces.len();
    while left < right {
        map_checkpoint_v2(checkpoint)?;
        charge_work_v2(work, 1, max_work)?;
        let middle = left + (right - left) / 2;
        match faces[middle].canonical_bytes().cmp(&key) {
            std::cmp::Ordering::Less => left = middle + 1,
            std::cmp::Ordering::Greater => right = middle,
            std::cmp::Ordering::Equal => return Ok(middle),
        }
    }
    Err(IntervalAttemptErrorV2::InvalidInput)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn recognize_exact_parallel_cut_with_checkpoint_v2(
    geometry: &MaterialHingeGraphGeometry,
    schedule: &CanonicalCycleScheduleV1,
    canonical_hinge_indices: &[usize],
    canonical_checked_hinges: &[EdgeId],
    max_work: usize,
    max_theorem_workspace_bytes: usize,
    checkpoint: &mut impl FnMut() -> Result<(), DyadicIntervalClosureStopV1>,
) -> Result<ExactParallelCutRecognitionV2, IntervalAttemptErrorV2> {
    let (profile, classifier_work) = schedule
        .classify_exact_parallel_cut_profile_with_checkpoint_v2(
            canonical_checked_hinges,
            max_work,
            &mut *checkpoint,
        )
        .map_err(|error| match error {
            ExactParallelCutProfileErrorV2::ResourceLimit => IntervalAttemptErrorV2::ResourceLimit,
            ExactParallelCutProfileErrorV2::Stop(DyadicIntervalClosureStopV1::Cancelled) => {
                IntervalAttemptErrorV2::Cancelled
            }
            ExactParallelCutProfileErrorV2::Stop(DyadicIntervalClosureStopV1::DeadlineExceeded) => {
                IntervalAttemptErrorV2::DeadlineExceeded
            }
        })?;
    let Some(profile) = profile else {
        return Ok(ExactParallelCutRecognitionV2::NotApplicable {
            charged_work: classifier_work,
            workspace_bytes: 0,
        });
    };
    let mut work = CanonicalCycleScheduleV1::exact_parallel_cut_profile_charged_work_v2(profile);

    let mut reference_position = None;
    for position in 0..canonical_hinge_indices.len() {
        map_checkpoint_v2(checkpoint)?;
        charge_work_v2(&mut work, 1, max_work)?;
        if schedule
            .exact_parallel_cut_position_is_moving_v2(profile, position)
            .ok_or(IntervalAttemptErrorV2::InvalidInput)?
        {
            reference_position = Some(position);
            break;
        }
    }
    let reference_position = reference_position.ok_or(IntervalAttemptErrorV2::InvalidInput)?;
    let reference = geometry
        .hinges()
        .get(canonical_hinge_indices[reference_position])
        .ok_or(IntervalAttemptErrorV2::InvalidInput)?;
    let axis = [
        reference.axis().x(),
        reference.axis().y(),
        reference.axis().z(),
    ];
    let Some(axis_dimension) = axis.iter().position(|coordinate| coordinate.abs() == 1.0) else {
        return Ok(ExactParallelCutRecognitionV2::NotApplicable {
            charged_work: work,
            workspace_bytes: 0,
        });
    };
    if axis
        .iter()
        .enumerate()
        .any(|(dimension, coordinate)| dimension != axis_dimension && *coordinate != 0.0)
    {
        return Ok(ExactParallelCutRecognitionV2::NotApplicable {
            charged_work: work,
            workspace_bytes: 0,
        });
    }

    let faces = geometry.face_ids().len();
    let logical_theorem_bytes = size_of::<u8>()
        .checked_mul(faces)
        .and_then(|bytes| size_of::<usize>().checked_mul(faces)?.checked_add(bytes))
        .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
    if logical_theorem_bytes > max_theorem_workspace_bytes {
        return Err(IntervalAttemptErrorV2::ResourceLimit);
    }
    let mut reached = Vec::<u8>::new();
    reached
        .try_reserve_exact(faces)
        .map_err(|_| IntervalAttemptErrorV2::ResourceLimit)?;
    let reached_bytes = size_of::<u8>()
        .checked_mul(reached.capacity())
        .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
    if reached_bytes
        .checked_add(
            size_of::<usize>()
                .checked_mul(faces)
                .ok_or(IntervalAttemptErrorV2::ResourceLimit)?,
        )
        .is_none_or(|bytes| bytes > max_theorem_workspace_bytes)
    {
        return Err(IntervalAttemptErrorV2::ResourceLimit);
    }
    for _ in 0..faces {
        map_checkpoint_v2(checkpoint)?;
        charge_work_v2(&mut work, 1, max_work)?;
        reached.push(0);
    }
    let mut queue = VecDeque::<usize>::new();
    queue
        .try_reserve(faces)
        .map_err(|_| IntervalAttemptErrorV2::ResourceLimit)?;
    let queue_bytes = size_of::<usize>()
        .checked_mul(queue.capacity())
        .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
    let theorem_workspace_bytes = reached_bytes
        .checked_add(queue_bytes)
        .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
    if theorem_workspace_bytes > max_theorem_workspace_bytes {
        return Err(IntervalAttemptErrorV2::ResourceLimit);
    }

    let reference_left = face_index_with_checkpoint_v2(
        geometry.face_ids(),
        reference.left_face(),
        &mut work,
        max_work,
        checkpoint,
    )?;
    reached[reference_left] = 1;
    queue.push_back(reference_left);
    let mut cursor = 0usize;
    while cursor < queue.len() {
        map_checkpoint_v2(checkpoint)?;
        let face = geometry.face_ids()[queue[cursor]];
        cursor = cursor
            .checked_add(1)
            .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
        for (position, geometry_index) in canonical_hinge_indices.iter().copied().enumerate() {
            map_checkpoint_v2(checkpoint)?;
            charge_work_v2(&mut work, 1, max_work)?;
            if schedule
                .exact_parallel_cut_position_is_moving_v2(profile, position)
                .ok_or(IntervalAttemptErrorV2::InvalidInput)?
            {
                continue;
            }
            let hinge = &geometry.hinges()[geometry_index];
            let neighbor = if hinge.left_face() == face {
                Some(hinge.right_face())
            } else if hinge.right_face() == face {
                Some(hinge.left_face())
            } else {
                None
            };
            let Some(neighbor) = neighbor else { continue };
            let neighbor_index = face_index_with_checkpoint_v2(
                geometry.face_ids(),
                neighbor,
                &mut work,
                max_work,
                checkpoint,
            )?;
            if reached[neighbor_index] == 0 {
                reached[neighbor_index] = 1;
                queue.push_back(neighbor_index);
            }
        }
    }

    let perpendicular = match axis_dimension {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    };
    let reference_start = [
        reference.start().x(),
        reference.start().y(),
        reference.start().z(),
    ];
    let mut effective_axis_reference = None;
    let mut moving_seen = 0usize;
    for (position, geometry_index) in canonical_hinge_indices.iter().copied().enumerate() {
        map_checkpoint_v2(checkpoint)?;
        charge_work_v2(&mut work, 1, max_work)?;
        let hinge = &geometry.hinges()[geometry_index];
        let left = face_index_with_checkpoint_v2(
            geometry.face_ids(),
            hinge.left_face(),
            &mut work,
            max_work,
            checkpoint,
        )?;
        let right = face_index_with_checkpoint_v2(
            geometry.face_ids(),
            hinge.right_face(),
            &mut work,
            max_work,
            checkpoint,
        )?;
        let moving = schedule
            .exact_parallel_cut_position_is_moving_v2(profile, position)
            .ok_or(IntervalAttemptErrorV2::InvalidInput)?;
        if moving != (reached[left] != reached[right]) {
            return Ok(ExactParallelCutRecognitionV2::NotApplicable {
                charged_work: work,
                workspace_bytes: theorem_workspace_bytes,
            });
        }
        if !moving {
            continue;
        }
        moving_seen = moving_seen
            .checked_add(1)
            .ok_or(IntervalAttemptErrorV2::ResourceLimit)?;
        let start = [hinge.start().x(), hinge.start().y(), hinge.start().z()];
        let end = [hinge.end().x(), hinge.end().y(), hinge.end().z()];
        if perpendicular.iter().any(|dimension| {
            start[*dimension].to_bits() != reference_start[*dimension].to_bits()
                || end[*dimension].to_bits() != reference_start[*dimension].to_bits()
        }) {
            return Ok(ExactParallelCutRecognitionV2::NotApplicable {
                charged_work: work,
                workspace_bytes: theorem_workspace_bytes,
            });
        }
        let assignment_sign = if hinge.assignment() == FoldAssignment::Mountain {
            1.0
        } else {
            -1.0
        };
        let traversal_sign = if reached[left] == 1 { 1.0 } else { -1.0 };
        let signed_axis = [
            assignment_sign * traversal_sign * hinge.axis().x(),
            assignment_sign * traversal_sign * hinge.axis().y(),
            assignment_sign * traversal_sign * hinge.axis().z(),
        ]
        .map(|coordinate| if coordinate == 0.0 { 0.0 } else { coordinate });
        if effective_axis_reference.is_some_and(|expected| expected != signed_axis) {
            return Ok(ExactParallelCutRecognitionV2::NotApplicable {
                charged_work: work,
                workspace_bytes: theorem_workspace_bytes,
            });
        }
        effective_axis_reference.get_or_insert(signed_axis);
    }
    if moving_seen == 0 {
        return Ok(ExactParallelCutRecognitionV2::NotApplicable {
            charged_work: work,
            workspace_bytes: theorem_workspace_bytes,
        });
    }
    Ok(ExactParallelCutRecognitionV2::Proven {
        charged_work: work,
        workspace_bytes: theorem_workspace_bytes,
    })
}
