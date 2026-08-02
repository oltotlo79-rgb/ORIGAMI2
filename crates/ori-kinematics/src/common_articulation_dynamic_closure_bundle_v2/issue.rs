use std::{mem::size_of, sync::Arc};

use sha2::Sha256;

use crate::schedule::{CycleScheduleRestrictionStopV1, CycleScheduleRestrictionWorkspaceErrorV2};
use crate::{
    CommonArticulationPoseInputV2, DyadicIntervalClosureControlErrorV1,
    DyadicIntervalClosureErrorV1, DyadicIntervalClosureStopV1,
};

use super::{
    binding::{binding_fingerprint_v2, geometry_audit_binding_v2},
    resources::{
        BundleValidationMeterV2, add_total, checked_bundle_retained_upper_bound_v2,
        checked_closure_nested_upper_bound_v2, checked_container_bytes_v2,
        checked_schedule_nested_upper_bound_v2, closure_retained_cap_from_bundle_remaining_v2,
        empty_resources_v2, ensure_revalidation_phase_fits_v2, invalid_resource_policy_v2,
        observe_issuance_peak_v2, remaining_issuance_phase_bytes_v2, resources_fit_limits_v2,
        schedule_retained_cap_from_bundle_remaining_v2,
    },
    *,
};

fn restriction_checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureBundleStopV2>,
) -> Result<(), CycleScheduleRestrictionStopV1> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationDynamicClosureBundleStopV2::Cancelled => {
            CycleScheduleRestrictionStopV1::Cancelled
        }
        CommonArticulationDynamicClosureBundleStopV2::DeadlineExceeded => {
            CycleScheduleRestrictionStopV1::DeadlineExceeded
        }
    })
}

fn closure_checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureBundleStopV2>,
) -> Result<(), DyadicIntervalClosureStopV1> {
    checkpoint().map_err(|stop| match stop {
        CommonArticulationDynamicClosureBundleStopV2::Cancelled => {
            DyadicIntervalClosureStopV1::Cancelled
        }
        CommonArticulationDynamicClosureBundleStopV2::DeadlineExceeded => {
            DyadicIntervalClosureStopV1::DeadlineExceeded
        }
    })
}

fn restriction_error_v2(
    error: CycleScheduleRestrictionWorkspaceErrorV2,
) -> CommonArticulationDynamicClosureBundleErrorV2 {
    match error {
        CycleScheduleRestrictionWorkspaceErrorV2::InvalidInput => {
            CommonArticulationDynamicClosureBundleErrorV2::InvalidInput
        }
        CycleScheduleRestrictionWorkspaceErrorV2::ResourceLimit => {
            CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit
        }
        CycleScheduleRestrictionWorkspaceErrorV2::Cancelled => {
            CommonArticulationDynamicClosureBundleErrorV2::Cancelled
        }
        CycleScheduleRestrictionWorkspaceErrorV2::DeadlineExceeded => {
            CommonArticulationDynamicClosureBundleErrorV2::DeadlineExceeded
        }
    }
}

fn closure_error_v2(
    error: DyadicIntervalClosureControlErrorV1,
) -> CommonArticulationDynamicClosureBundleErrorV2 {
    match error {
        DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::ResourceLimit,
        ) => CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit,
        DyadicIntervalClosureControlErrorV1::Closure(
            DyadicIntervalClosureErrorV1::UnprovenClosure { depth, index },
        ) => CommonArticulationDynamicClosureBundleErrorV2::UnprovenClosure { depth, index },
        DyadicIntervalClosureControlErrorV1::Closure(_) => {
            CommonArticulationDynamicClosureBundleErrorV2::InvalidInput
        }
        DyadicIntervalClosureControlErrorV1::Cancelled => {
            CommonArticulationDynamicClosureBundleErrorV2::Cancelled
        }
        DyadicIntervalClosureControlErrorV1::DeadlineExceeded => {
            CommonArticulationDynamicClosureBundleErrorV2::DeadlineExceeded
        }
    }
}

fn canonical_block_fixed_face_v2(
    geometry: &MaterialHingeGraphGeometry,
    articulation_faces: &[FaceId],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureBundleStopV2>,
    meter: &mut BundleValidationMeterV2,
) -> Result<FaceId, CommonArticulationDynamicClosureBundleErrorV2> {
    let mut selected = None;
    for face in geometry.face_ids() {
        meter.poll(checkpoint)?;
        let mut articulation = false;
        for candidate in articulation_faces {
            meter.poll(checkpoint)?;
            if candidate == face {
                articulation = true;
                break;
            }
        }
        if articulation
            && selected
                .is_none_or(|current: FaceId| face.canonical_bytes() < current.canonical_bytes())
        {
            selected = Some(*face);
        }
    }
    selected.ok_or(CommonArticulationDynamicClosureBundleErrorV2::InvalidInput)
}

/// Crate-private issuer used by the future transport layer. No legacy closure
/// observation or pose authority is manufactured on this path.
#[allow(dead_code)] // Phase 3 transport connects this sealed issuer.
pub(crate) fn prove_common_articulation_dynamic_closure_bundle_with_checkpoint_v2(
    input: CommonArticulationDynamicClosureBundleInputV2<'_>,
    mut checkpoint: impl FnMut() -> Result<(), CommonArticulationDynamicClosureBundleStopV2>,
) -> Result<CommonArticulationDynamicClosureBundleV2, CommonArticulationDynamicClosureBundleErrorV2>
{
    issue_v2(input, &mut checkpoint, 0)
}

pub(super) fn issue_v2(
    input: CommonArticulationDynamicClosureBundleInputV2<'_>,
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationDynamicClosureBundleStopV2>,
    retained_revalidation_offset: usize,
) -> Result<CommonArticulationDynamicClosureBundleV2, CommonArticulationDynamicClosureBundleErrorV2>
{
    checkpoint_v2(checkpoint)?;
    if !input.paper_thickness_mm.is_finite()
        || input.paper_thickness_mm <= 0.0
        || !input.closure_tolerance.is_finite()
        || input.closure_tolerance < 0.0
        || input.closure_tolerance.to_bits() == (-0.0_f64).to_bits()
        || input.limits.per_block_closure_limits.max_depth >= 64
        || input.limits.parent_closure_limits.max_depth >= 64
    {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::InvalidInput);
    }
    if invalid_resource_policy_v2(input.limits) {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit);
    }
    let mut validation_meter = BundleValidationMeterV2::new(input.limits.max_validation_work);
    let configured_max_blocks = input.profile.configured_max_blocks_v2();
    let actual_block_count = input.profile.actual_block_count_v2();
    let actual = input.profile.actual_v2();
    let maximum = input.profile.maximum_v2();
    let face_count = input.geometry.face_ids().len();
    let hinge_count = input.geometry.hinges().len();
    if configured_max_blocks < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count < GENERAL_N_MIN_BLOCKS_V2
        || actual_block_count > configured_max_blocks
        || input.limits.max_blocks != configured_max_blocks
        || face_count != actual.face_count_v2()
        || hinge_count != actual.hinge_count_v2()
        || face_count > maximum.face_count_v2()
        || hinge_count > maximum.hinge_count_v2()
        || input.decomposition.actual_block_count_v2() != actual_block_count
        || input.decomposition.face_count_v2() != face_count
        || input.decomposition.hinge_count_v2() != hinge_count
        || input.decomposition.blocks().len() != actual_block_count
        || !input.decomposition.is_for_geometry(input.geometry)
        || !input.decomposition.is_for_profile_v2(input.profile)
        || input.pose.fixed_face() != input.parent_fixed_face
    {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::InvalidInput);
    }
    let pose_matches = input.common_pose.matches_live_input_with_checkpoint_v2(
        CommonArticulationPoseInputV2 {
            geometry: input.geometry,
            pose: input.pose,
            decomposition: input.decomposition,
            paper_thickness_mm: input.paper_thickness_mm,
            profile: input.profile,
        },
        || validation_meter.poll(checkpoint),
    )?;
    if !pose_matches {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::IssuerMismatch);
    }
    let schedule_binding_matches = input.parent_schedule.matches_binding_with_checkpoint_v2(
        input.geometry,
        input.audit,
        input.parent_fixed_face,
        &mut || validation_meter.poll(checkpoint),
    )?;
    if !schedule_binding_matches
        || !input
            .parent_schedule
            .matches_hinge_angles_at_parameter_with_checkpoint_v2(
                0.0,
                input.pose.hinge_angles(),
                || validation_meter.poll(checkpoint),
            )?
    {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::InvalidInput);
    }
    let audit_binding = geometry_audit_binding_v2(
        input.geometry,
        input.audit,
        checkpoint,
        &mut validation_meter,
    )?;
    let mut parent_fixed_face_present = false;
    for face in input.audit.faces() {
        validation_meter.poll(checkpoint)?;
        if *face == input.parent_fixed_face {
            parent_fixed_face_present = true;
            break;
        }
    }
    if !parent_fixed_face_present {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::InvalidInput);
    }

    let logical_block_record_bytes = size_of::<DynamicBlockClosureRecordV2>()
        .checked_mul(actual_block_count)
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    let logical_container_bytes = checked_container_bytes_v2(actual_block_count)
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    if logical_block_record_bytes > input.limits.max_block_record_bytes
        || logical_container_bytes > input.limits.max_bundle_retained_bytes
    {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit);
    }
    ensure_revalidation_phase_fits_v2(
        logical_container_bytes,
        retained_revalidation_offset,
        input.limits,
    )?;
    let mut blocks = Vec::<DynamicBlockClosureRecordV2>::new();
    blocks
        .try_reserve_exact(actual_block_count)
        .map_err(|_| CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    let block_record_bytes = size_of::<DynamicBlockClosureRecordV2>()
        .checked_mul(blocks.capacity())
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    if block_record_bytes > input.limits.max_block_record_bytes {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit);
    }
    let container_bytes = checked_container_bytes_v2(blocks.capacity())
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    if container_bytes > input.limits.max_bundle_retained_bytes {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit);
    }
    let mut resources = empty_resources_v2(block_record_bytes);
    resources.charged_validation_work = validation_meter.work;
    observe_issuance_peak_v2(
        &mut resources,
        container_bytes,
        input.limits,
        retained_revalidation_offset,
    )?;
    let mut accumulated_block_nested = 0usize;

    for (block_index, block) in input.decomposition.blocks().iter().enumerate() {
        validation_meter.poll(checkpoint)?;
        let block_geometry = block.geometry();
        let block_audit = block.audit();
        let fixed_face = canonical_block_fixed_face_v2(
            block_geometry,
            input.decomposition.articulation_faces(),
            checkpoint,
            &mut validation_meter,
        )?;
        let geometry_audit_binding = geometry_audit_binding_v2(
            block_geometry,
            block_audit,
            checkpoint,
            &mut validation_meter,
        )?;
        resources.charged_validation_work = validation_meter.work;
        let restriction_outer_live = container_bytes
            .checked_add(accumulated_block_nested)
            .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
        let mut effective_restriction_limits = input.limits.block_restriction_limits;
        effective_restriction_limits.max_work = effective_restriction_limits.max_work.min(
            input
                .limits
                .max_total_restriction_work
                .checked_sub(resources.charged_total_restriction_work)
                .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?,
        );
        effective_restriction_limits.max_restricted_schedule_retained_bytes =
            effective_restriction_limits
                .max_restricted_schedule_retained_bytes
                .min(
                    input
                        .limits
                        .max_total_restricted_schedule_retained_bytes
                        .checked_sub(
                            resources.charged_total_restricted_schedule_retained_upper_bound_bytes,
                        )
                        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?,
                )
                .min(schedule_retained_cap_from_bundle_remaining_v2(
                    restriction_outer_live,
                    input.limits,
                )?);
        effective_restriction_limits.max_restriction_peak_bytes = effective_restriction_limits
            .max_restriction_peak_bytes
            .min(remaining_issuance_phase_bytes_v2(
                restriction_outer_live,
                retained_revalidation_offset,
                input.limits,
            )?);
        let restricted = input
            .parent_schedule
            .restrict_to_edge_block_with_workspace_and_checkpoint_v2(
                input.geometry,
                input.audit,
                block_geometry,
                block_audit,
                fixed_face,
                effective_restriction_limits,
                || restriction_checkpoint_v2(checkpoint),
            )
            .map_err(restriction_error_v2)?;
        let restriction_resources = restricted.resources;
        let restricted_schedule = restricted.schedule;
        add_total(
            &mut resources.charged_total_restriction_work,
            restriction_resources.charged_work,
        )?;
        add_total(
            &mut resources.charged_total_restricted_schedule_retained_upper_bound_bytes,
            restriction_resources.charged_restricted_schedule_retained_upper_bound_bytes,
        )?;
        resources.charged_max_block_restriction_peak_upper_bound_bytes = resources
            .charged_max_block_restriction_peak_upper_bound_bytes
            .max(restriction_resources.charged_restriction_peak_upper_bound_bytes);
        let restriction_phase = container_bytes
            .checked_add(accumulated_block_nested)
            .and_then(|bytes| {
                bytes.checked_add(restriction_resources.charged_restriction_peak_upper_bound_bytes)
            })
            .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
        observe_issuance_peak_v2(
            &mut resources,
            restriction_phase,
            input.limits,
            retained_revalidation_offset,
        )?;
        if !resources_fit_limits_v2(resources, input.limits) {
            return Err(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit);
        }

        let schedule_nested = checked_schedule_nested_upper_bound_v2(restriction_resources)
            .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
        let closure_outer_peak_live = restriction_outer_live
            .checked_add(
                restriction_resources.charged_restricted_schedule_retained_upper_bound_bytes,
            )
            .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
        let closure_outer_retained_live = restriction_outer_live
            .checked_add(schedule_nested)
            .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
        let mut effective_closure_limits = input.limits.per_block_closure_limits;
        effective_closure_limits.max_leaves = effective_closure_limits.max_leaves.min(
            input
                .limits
                .max_total_block_leaves
                .checked_sub(resources.charged_total_block_leaves)
                .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?,
        );
        effective_closure_limits.max_retained_material_bytes = effective_closure_limits
            .max_retained_material_bytes
            .min(
                input
                    .limits
                    .max_total_block_closure_retained_bytes
                    .checked_sub(resources.charged_total_block_closure_retained_upper_bound_bytes)
                    .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?,
            )
            .min(closure_retained_cap_from_bundle_remaining_v2(
                closure_outer_retained_live,
                input.limits,
            )?);
        effective_closure_limits.max_peak_workspace_bytes = effective_closure_limits
            .max_peak_workspace_bytes
            .min(remaining_issuance_phase_bytes_v2(
                closure_outer_peak_live,
                retained_revalidation_offset,
                input.limits,
            )?);
        let closure = block_geometry
            .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
                block_audit,
                fixed_face,
                &restricted_schedule,
                input.closure_tolerance,
                effective_closure_limits,
                || closure_checkpoint_v2(checkpoint),
            )
            .map_err(closure_error_v2)?;
        if closure.resources().issued_leaves == 0
            || closure.partition().len() != closure.resources().issued_leaves
            || closure.canonical_checked_hinges().len() != block_geometry.hinges().len()
        {
            return Err(CommonArticulationDynamicClosureBundleErrorV2::InvalidInput);
        }
        let closure_resources = closure.resources();
        add_total(
            &mut resources.charged_total_block_closure_retained_upper_bound_bytes,
            closure_resources.charged_retained_material_upper_bound_bytes,
        )?;
        add_total(
            &mut resources.charged_total_block_leaves,
            closure_resources.issued_leaves,
        )?;
        resources.charged_max_block_closure_peak_upper_bound_bytes = resources
            .charged_max_block_closure_peak_upper_bound_bytes
            .max(closure_resources.charged_peak_workspace_upper_bound_bytes);
        let closure_phase = container_bytes
            .checked_add(accumulated_block_nested)
            .and_then(|bytes| {
                bytes.checked_add(
                    restriction_resources.charged_restricted_schedule_retained_upper_bound_bytes,
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(closure_resources.charged_peak_workspace_upper_bound_bytes)
            })
            .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
        observe_issuance_peak_v2(
            &mut resources,
            closure_phase,
            input.limits,
            retained_revalidation_offset,
        )?;
        if !resources_fit_limits_v2(resources, input.limits) {
            return Err(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit);
        }
        accumulated_block_nested = accumulated_block_nested
            .checked_add(schedule_nested)
            .and_then(|bytes| {
                checked_closure_nested_upper_bound_v2(closure_resources)
                    .and_then(|nested| bytes.checked_add(nested))
            })
            .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
        blocks.push(DynamicBlockClosureRecordV2 {
            block_index,
            issuer_geometry: block_geometry.instance_anchor_v1(),
            fixed_face,
            geometry_audit_binding,
            restricted_schedule,
            restriction_resources,
            closure,
        });
    }
    if blocks.len() != actual_block_count {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::InvalidInput);
    }

    let parent_restriction_outer_live = container_bytes
        .checked_add(accumulated_block_nested)
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    let mut effective_parent_restriction_limits = input.limits.parent_schedule_restriction_limits;
    effective_parent_restriction_limits.max_work =
        effective_parent_restriction_limits.max_work.min(
            input
                .limits
                .max_total_restriction_work
                .checked_sub(resources.charged_total_restriction_work)
                .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?,
        );
    effective_parent_restriction_limits.max_restricted_schedule_retained_bytes =
        effective_parent_restriction_limits
            .max_restricted_schedule_retained_bytes
            .min(input.limits.max_parent_schedule_retained_bytes)
            .min(schedule_retained_cap_from_bundle_remaining_v2(
                parent_restriction_outer_live,
                input.limits,
            )?);
    effective_parent_restriction_limits.max_restriction_peak_bytes =
        effective_parent_restriction_limits
            .max_restriction_peak_bytes
            .min(remaining_issuance_phase_bytes_v2(
                parent_restriction_outer_live,
                retained_revalidation_offset,
                input.limits,
            )?);
    let owned_parent = input
        .parent_schedule
        .restrict_to_edge_block_with_workspace_and_checkpoint_v2(
            input.geometry,
            input.audit,
            input.geometry,
            input.audit,
            input.parent_fixed_face,
            effective_parent_restriction_limits,
            || restriction_checkpoint_v2(checkpoint),
        )
        .map_err(restriction_error_v2)?;
    let parent_schedule_restriction_resources = owned_parent.resources;
    let parent_schedule = owned_parent.schedule;
    add_total(
        &mut resources.charged_total_restriction_work,
        parent_schedule_restriction_resources.charged_work,
    )?;
    resources.charged_parent_schedule_retained_upper_bound_bytes =
        parent_schedule_restriction_resources
            .charged_restricted_schedule_retained_upper_bound_bytes;
    resources.charged_parent_schedule_restriction_peak_upper_bound_bytes =
        parent_schedule_restriction_resources.charged_restriction_peak_upper_bound_bytes;
    let parent_restriction_phase = container_bytes
        .checked_add(accumulated_block_nested)
        .and_then(|bytes| {
            bytes.checked_add(
                parent_schedule_restriction_resources.charged_restriction_peak_upper_bound_bytes,
            )
        })
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    observe_issuance_peak_v2(
        &mut resources,
        parent_restriction_phase,
        input.limits,
        retained_revalidation_offset,
    )?;
    if !resources_fit_limits_v2(resources, input.limits) {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit);
    }

    let parent_schedule_nested =
        checked_schedule_nested_upper_bound_v2(parent_schedule_restriction_resources)
            .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    let parent_closure_outer_peak_live = parent_restriction_outer_live
        .checked_add(
            parent_schedule_restriction_resources
                .charged_restricted_schedule_retained_upper_bound_bytes,
        )
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    let parent_closure_outer_retained_live = parent_restriction_outer_live
        .checked_add(parent_schedule_nested)
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    let mut effective_parent_closure_limits = input.limits.parent_closure_limits;
    effective_parent_closure_limits.max_leaves = effective_parent_closure_limits
        .max_leaves
        .min(input.limits.max_parent_leaves);
    effective_parent_closure_limits.max_retained_material_bytes = effective_parent_closure_limits
        .max_retained_material_bytes
        .min(input.limits.max_parent_closure_retained_bytes)
        .min(closure_retained_cap_from_bundle_remaining_v2(
            parent_closure_outer_retained_live,
            input.limits,
        )?);
    effective_parent_closure_limits.max_peak_workspace_bytes = effective_parent_closure_limits
        .max_peak_workspace_bytes
        .min(remaining_issuance_phase_bytes_v2(
            parent_closure_outer_peak_live,
            retained_revalidation_offset,
            input.limits,
        )?);
    let parent_closure = input
        .geometry
        .prove_dyadic_schedule_closure_with_workspace_and_checkpoint_v2(
            input.audit,
            input.parent_fixed_face,
            &parent_schedule,
            input.closure_tolerance,
            effective_parent_closure_limits,
            || closure_checkpoint_v2(checkpoint),
        )
        .map_err(closure_error_v2)?;
    if parent_closure.resources().issued_leaves == 0
        || parent_closure.partition().len() != parent_closure.resources().issued_leaves
        || parent_closure.canonical_checked_hinges().len() != hinge_count
    {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::InvalidInput);
    }
    let parent_closure_resources = parent_closure.resources();
    resources.charged_parent_closure_retained_upper_bound_bytes =
        parent_closure_resources.charged_retained_material_upper_bound_bytes;
    resources.charged_parent_leaves = parent_closure_resources.issued_leaves;
    resources.charged_parent_closure_peak_upper_bound_bytes =
        parent_closure_resources.charged_peak_workspace_upper_bound_bytes;
    let parent_closure_phase = container_bytes
        .checked_add(accumulated_block_nested)
        .and_then(|bytes| {
            bytes.checked_add(
                parent_schedule_restriction_resources
                    .charged_restricted_schedule_retained_upper_bound_bytes,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(parent_closure_resources.charged_peak_workspace_upper_bound_bytes)
        })
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    observe_issuance_peak_v2(
        &mut resources,
        parent_closure_phase,
        input.limits,
        retained_revalidation_offset,
    )?;

    let bundle_retained = checked_bundle_retained_upper_bound_v2(
        blocks.capacity(),
        accumulated_block_nested,
        parent_schedule_restriction_resources,
        parent_closure_resources,
    )
    .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    resources.charged_bundle_retained_upper_bound_bytes = bundle_retained;
    let publication_peak = bundle_retained
        .checked_add(size_of::<Sha256>())
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    observe_issuance_peak_v2(
        &mut resources,
        publication_peak,
        input.limits,
        retained_revalidation_offset,
    )?;
    resources.charged_revalidation_peak_upper_bound_bytes = bundle_retained
        .checked_add(resources.charged_issuance_peak_upper_bound_bytes)
        .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?;
    // Reserve all remaining validation work before publication. `2B + 4`
    // covers binding publication plus the issuer's final block checks; the
    // additional `B + 1` covers revalidation's sealed bundle comparison.
    validation_meter.charge(
        actual_block_count
            .checked_mul(3)
            .and_then(|work| work.checked_add(5))
            .ok_or(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit)?,
    )?;
    resources.charged_validation_work = validation_meter.work;
    if !resources_fit_limits_v2(resources, input.limits) {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::ResourceLimit);
    }
    let binding_fingerprint = binding_fingerprint_v2(
        input,
        audit_binding,
        &blocks,
        &parent_schedule,
        parent_schedule_restriction_resources,
        &parent_closure,
        resources,
        checkpoint,
    )?;
    let bundle = CommonArticulationDynamicClosureBundleV2 {
        issuer_geometry: input.geometry.instance_anchor_v1(),
        issuer_pose: input.pose.instance_anchor_v2(),
        profile_binding: input.profile.binding_fingerprint_v2(),
        decomposition_binding: input.decomposition.binding_fingerprint_v2(),
        common_pose_binding: input.common_pose.binding_fingerprint_v2(),
        audit_binding,
        parent_schedule_binding: input.parent_schedule.certificate_binding_fingerprint_v2(),
        parent_fixed_face: input.parent_fixed_face,
        paper_thickness_bits: input.paper_thickness_mm.to_bits(),
        closure_tolerance_bits: input.closure_tolerance.to_bits(),
        configured_max_blocks,
        actual_block_count,
        face_count,
        hinge_count,
        policy: input.limits,
        blocks,
        parent_schedule,
        parent_schedule_restriction_resources,
        parent_closure,
        resources,
        binding_fingerprint,
    };
    if !bundle.issuer_geometry.matches(input.geometry)
        || !Arc::ptr_eq(&bundle.issuer_pose, &input.pose.instance_anchor_v2())
        || bundle.profile_binding != input.profile.binding_fingerprint_v2()
        || bundle.decomposition_binding != input.decomposition.binding_fingerprint_v2()
        || bundle.common_pose_binding != input.common_pose.binding_fingerprint_v2()
        || bundle.audit_binding != audit_binding
        || bundle.parent_schedule_binding
            != input.parent_schedule.certificate_binding_fingerprint_v2()
        || bundle.parent_fixed_face != input.parent_fixed_face
        || bundle.paper_thickness_bits != input.paper_thickness_mm.to_bits()
        || bundle.closure_tolerance_bits != input.closure_tolerance.to_bits()
        || bundle.configured_max_blocks != configured_max_blocks
        || bundle.actual_block_count != actual_block_count
        || bundle.face_count != face_count
        || bundle.hinge_count != hinge_count
        || bundle.policy != input.limits
        || bundle.blocks.len() != actual_block_count
        || bundle.resources() != resources
        || bundle.binding_fingerprint_v2() != binding_fingerprint
        || bundle.parent_leaf_descriptor(0).is_none()
    {
        return Err(CommonArticulationDynamicClosureBundleErrorV2::InvalidInput);
    }
    for index in 0..actual_block_count {
        checkpoint_v2(checkpoint)?;
        if bundle.block_leaf_descriptor(index, 0).is_none() {
            return Err(CommonArticulationDynamicClosureBundleErrorV2::InvalidInput);
        }
    }
    checkpoint_v2(checkpoint)?;
    Ok(bundle)
}
