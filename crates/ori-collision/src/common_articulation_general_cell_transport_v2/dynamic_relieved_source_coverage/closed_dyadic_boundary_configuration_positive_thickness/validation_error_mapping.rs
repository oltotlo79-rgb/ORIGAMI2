//! Private translations across the Phase 3I delegation boundaries.

use ori_kinematics::{
    CycleScheduleClosedDyadicBoundaryErrorV2, CycleScheduleClosedDyadicBoundaryStopV2,
    CycleSchedulePrepareErrorV1,
};

use super::{ErrorV2, StopV2};
use crate::{
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
    CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2,
};

pub(super) fn checkpoint_v2(
    checkpoint: &mut impl FnMut() -> Result<(), StopV2>,
) -> Result<(), ErrorV2> {
    checkpoint().map_err(map_stop_v2)
}

const fn map_stop_v2(stop: StopV2) -> ErrorV2 {
    match stop {
        StopV2::Cancelled => ErrorV2::Cancelled,
        StopV2::DeadlineExceeded => ErrorV2::DeadlineExceeded,
    }
}

pub(super) const fn map_stop_to_boundary_v2(
    stop: StopV2,
) -> CycleScheduleClosedDyadicBoundaryStopV2 {
    match stop {
        StopV2::Cancelled => CycleScheduleClosedDyadicBoundaryStopV2::Cancelled,
        StopV2::DeadlineExceeded => CycleScheduleClosedDyadicBoundaryStopV2::DeadlineExceeded,
    }
}

pub(super) const fn map_boundary_error_v2(
    error: CycleScheduleClosedDyadicBoundaryErrorV2,
) -> ErrorV2 {
    match error {
        CycleScheduleClosedDyadicBoundaryErrorV2::Prepare(
            CycleSchedulePrepareErrorV1::ResourceLimit,
        )
        | CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit => ErrorV2::ResourceLimit,
        CycleScheduleClosedDyadicBoundaryErrorV2::Prepare(_) => {
            ErrorV2::BoundaryConfigurationUnavailable
        }
        CycleScheduleClosedDyadicBoundaryErrorV2::Cancelled => ErrorV2::Cancelled,
        CycleScheduleClosedDyadicBoundaryErrorV2::DeadlineExceeded => ErrorV2::DeadlineExceeded,
    }
}

pub(super) const fn map_stop_to_endpoint_v2(
    stop: StopV2,
) -> CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2 {
    match stop {
        StopV2::Cancelled => {
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2::Cancelled
        }
        StopV2::DeadlineExceeded => {
            CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteStopV2::DeadlineExceeded
        }
    }
}

pub(super) const fn map_endpoint_error_v2(
    error: CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2,
) -> ErrorV2 {
    match error {
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::Cancelled => {
            ErrorV2::Cancelled
        }
        CommonArticulationDynamicGeneralNClosedDyadicEndpointPositiveThicknessPrerequisiteErrorV2::DeadlineExceeded => {
            ErrorV2::DeadlineExceeded
        }
        other => ErrorV2::EndpointPositiveThickness(other),
    }
}
