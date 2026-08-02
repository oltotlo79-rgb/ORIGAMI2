use super::*;

const ENDPOINT_DOMAIN_SEPARATOR_V2: &[u8] =
    b"ORIGAMI2_CANONICAL_CYCLE_SCHEDULE_CLOSED_DYADIC_BOUNDARY_ENDPOINT_V2";
const EVIDENCE_DOMAIN_SEPARATOR_V2: &[u8] =
    b"ORIGAMI2_CANONICAL_CYCLE_SCHEDULE_CLOSED_DYADIC_BOUNDARY_EVIDENCE_V2";
const LOWER_ENDPOINT_TAG_V2: u8 = 0;
const UPPER_ENDPOINT_TAG_V2: u8 = 1;
const ORDINARY_ANGLE_RECORD_TAG_V2: u8 = 0;
const HALF_ANGLE_BOX_RECORD_TAG_V2: u8 = 1;

pub(super) struct EndpointBindingHasherV2 {
    hash: Sha256,
}

impl EndpointBindingHasherV2 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new_v2(
        representation: BoundaryRepresentationV2,
        upper: bool,
        schedule_binding: [u8; 32],
        graph_binding: [u8; 32],
        hinge_count: usize,
        limits: CycleScheduleLimitsV1,
        meter: &mut resources::BoundaryWorkMeterV2,
    ) -> Result<Self, CycleScheduleClosedDyadicBoundaryErrorV2> {
        let mut hash = Sha256::new();
        update_frame_metered_v2(&mut hash, ENDPOINT_DOMAIN_SEPARATOR_V2, meter)?;
        update_frame_metered_v2(
            &mut hash,
            CANONICAL_CYCLE_SCHEDULE_CLOSED_DYADIC_BOUNDARY_EVIDENCE_MODEL_ID_V2.as_bytes(),
            meter,
        )?;
        update_frame_metered_v2(
            &mut hash,
            CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2.as_bytes(),
            meter,
        )?;
        update_frame_metered_v2(
            &mut hash,
            DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.as_bytes(),
            meter,
        )?;
        update_frame_metered_v2(&mut hash, &[representation.tag_v2()], meter)?;
        update_frame_metered_v2(
            &mut hash,
            &[if upper {
                UPPER_ENDPOINT_TAG_V2
            } else {
                LOWER_ENDPOINT_TAG_V2
            }],
            meter,
        )?;
        update_frame_metered_v2(&mut hash, &schedule_binding, meter)?;
        update_frame_metered_v2(&mut hash, &graph_binding, meter)?;
        update_usize_metered_v2(&mut hash, hinge_count, meter)?;
        update_limits_metered_v2(&mut hash, limits, meter)?;
        Ok(Self { hash })
    }

    pub(super) fn update_ordinary_v2(
        &mut self,
        edge: EdgeId,
        angle_bits: u64,
        meter: &mut resources::BoundaryWorkMeterV2,
    ) -> Result<(), CycleScheduleClosedDyadicBoundaryErrorV2> {
        update_frame_metered_v2(&mut self.hash, &edge.canonical_bytes(), meter)?;
        update_frame_metered_v2(&mut self.hash, &[ORDINARY_ANGLE_RECORD_TAG_V2], meter)?;
        update_frame_metered_v2(&mut self.hash, &angle_bits.to_be_bytes(), meter)
    }

    pub(super) fn update_half_angle_v2(
        &mut self,
        edge: EdgeId,
        lower_bits: u64,
        upper_bits: u64,
        meter: &mut resources::BoundaryWorkMeterV2,
    ) -> Result<(), CycleScheduleClosedDyadicBoundaryErrorV2> {
        update_frame_metered_v2(&mut self.hash, &edge.canonical_bytes(), meter)?;
        update_frame_metered_v2(&mut self.hash, &[HALF_ANGLE_BOX_RECORD_TAG_V2], meter)?;
        update_frame_metered_v2(&mut self.hash, &lower_bits.to_be_bytes(), meter)?;
        update_frame_metered_v2(&mut self.hash, &upper_bits.to_be_bytes(), meter)
    }

    pub(super) fn finalize_v2(self) -> [u8; 32] {
        self.hash.finalize().into()
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn evidence_binding_fingerprint_v2(
    representation: BoundaryRepresentationV2,
    schedule_binding: [u8; 32],
    graph_binding: [u8; 32],
    lower_binding: [u8; 32],
    upper_binding: [u8; 32],
    hinge_count: usize,
    limits: CycleScheduleLimitsV1,
    logical_work: usize,
    workspace_peak_bytes: usize,
) -> Result<[u8; 32], CycleScheduleClosedDyadicBoundaryErrorV2> {
    let mut hash = Sha256::new();
    update_frame_v2(&mut hash, EVIDENCE_DOMAIN_SEPARATOR_V2)?;
    update_frame_v2(
        &mut hash,
        CANONICAL_CYCLE_SCHEDULE_CLOSED_DYADIC_BOUNDARY_EVIDENCE_MODEL_ID_V2.as_bytes(),
    )?;
    update_frame_v2(&mut hash, CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2.as_bytes())?;
    update_frame_v2(
        &mut hash,
        DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.as_bytes(),
    )?;
    update_frame_v2(&mut hash, &[representation.tag_v2()])?;
    update_frame_v2(&mut hash, &schedule_binding)?;
    update_frame_v2(&mut hash, &graph_binding)?;
    update_frame_v2(&mut hash, &[LOWER_ENDPOINT_TAG_V2])?;
    update_frame_v2(&mut hash, &lower_binding)?;
    update_frame_v2(&mut hash, &[UPPER_ENDPOINT_TAG_V2])?;
    update_frame_v2(&mut hash, &upper_binding)?;
    update_usize_v2(&mut hash, hinge_count)?;
    update_limits_v2(&mut hash, limits)?;
    update_usize_v2(&mut hash, logical_work)?;
    update_usize_v2(&mut hash, workspace_peak_bytes)?;
    Ok(hash.finalize().into())
}

pub(super) fn checked_endpoint_binding_work_v2(
    representation: BoundaryRepresentationV2,
    hinge_count: usize,
) -> Option<usize> {
    let fixed = [
        ENDPOINT_DOMAIN_SEPARATOR_V2.len(),
        CANONICAL_CYCLE_SCHEDULE_CLOSED_DYADIC_BOUNDARY_EVIDENCE_MODEL_ID_V2.len(),
        CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2.len(),
        DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.len(),
        1,
        1,
        32,
        32,
        8,
        8,
        8,
        4,
        8,
    ]
    .into_iter()
    .try_fold(0usize, |work, length| {
        work.checked_add(checked_frame_work_v2(length)?)
    })?;
    let record_lengths: &[usize] = match representation {
        BoundaryRepresentationV2::Ordinary => &[16, 1, 8],
        BoundaryRepresentationV2::HalfAngle => &[16, 1, 8, 8],
    };
    let record = record_lengths.iter().try_fold(0usize, |work, length| {
        work.checked_add(checked_frame_work_v2(*length)?)
    })?;
    fixed.checked_add(record.checked_mul(hinge_count)?)
}

pub(super) fn checked_evidence_binding_work_v2() -> usize {
    [
        EVIDENCE_DOMAIN_SEPARATOR_V2.len(),
        CANONICAL_CYCLE_SCHEDULE_CLOSED_DYADIC_BOUNDARY_EVIDENCE_MODEL_ID_V2.len(),
        CANONICAL_CYCLE_SCHEDULE_MODEL_ID_V2.len(),
        DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1.len(),
        1,
        32,
        32,
        1,
        32,
        1,
        32,
        8,
        8,
        8,
        4,
        8,
        8,
        8,
    ]
    .into_iter()
    .map(|length| checked_frame_work_v2(length).expect("fixed frame work must fit usize"))
    .sum()
}

fn update_limits_metered_v2(
    hash: &mut Sha256,
    limits: CycleScheduleLimitsV1,
    meter: &mut resources::BoundaryWorkMeterV2,
) -> Result<(), CycleScheduleClosedDyadicBoundaryErrorV2> {
    update_usize_metered_v2(hash, limits.max_hinges, meter)?;
    update_usize_metered_v2(hash, limits.max_degree, meter)?;
    update_frame_metered_v2(hash, &limits.max_coefficient_bits.to_be_bytes(), meter)?;
    update_usize_metered_v2(hash, limits.max_work, meter)
}

fn update_limits_v2(
    hash: &mut Sha256,
    limits: CycleScheduleLimitsV1,
) -> Result<(), CycleScheduleClosedDyadicBoundaryErrorV2> {
    update_usize_v2(hash, limits.max_hinges)?;
    update_usize_v2(hash, limits.max_degree)?;
    update_frame_v2(hash, &limits.max_coefficient_bits.to_be_bytes())?;
    update_usize_v2(hash, limits.max_work)
}

fn update_usize_metered_v2(
    hash: &mut Sha256,
    value: usize,
    meter: &mut resources::BoundaryWorkMeterV2,
) -> Result<(), CycleScheduleClosedDyadicBoundaryErrorV2> {
    let value = u64::try_from(value)
        .map_err(|_| CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
    update_frame_metered_v2(hash, &value.to_be_bytes(), meter)
}

fn update_usize_v2(
    hash: &mut Sha256,
    value: usize,
) -> Result<(), CycleScheduleClosedDyadicBoundaryErrorV2> {
    let value = u64::try_from(value)
        .map_err(|_| CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
    update_frame_v2(hash, &value.to_be_bytes())
}

fn update_frame_metered_v2(
    hash: &mut Sha256,
    value: &[u8],
    meter: &mut resources::BoundaryWorkMeterV2,
) -> Result<(), CycleScheduleClosedDyadicBoundaryErrorV2> {
    meter.charge_v2(
        checked_frame_work_v2(value.len())
            .ok_or(CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?,
    )?;
    update_frame_v2(hash, value)
}

fn update_frame_v2(
    hash: &mut Sha256,
    value: &[u8],
) -> Result<(), CycleScheduleClosedDyadicBoundaryErrorV2> {
    let length = u64::try_from(value.len())
        .map_err(|_| CycleScheduleClosedDyadicBoundaryErrorV2::ResourceLimit)?;
    hash.update(length.to_be_bytes());
    hash.update(value);
    Ok(())
}

const fn checked_frame_work_v2(length: usize) -> Option<usize> {
    8usize.checked_add(length)
}
