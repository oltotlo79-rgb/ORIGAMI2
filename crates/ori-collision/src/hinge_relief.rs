use std::collections::HashSet;

use num_rational::BigRational;
use num_traits::FromPrimitive;
use ori_domain::EdgeId;
use ori_kinematics::MaterialHingeGraphGeometry;
use thiserror::Error;

pub const HINGE_RELIEF_POLICY_MODEL_ID_V1: &str = "explicit_hinge_relief_prerequisite_v1";
pub const MAX_HINGE_RELIEF_RECORDS_V1: usize = 256;
pub const MAX_HINGE_RELIEF_EXACT_BITS_PER_RECORD_V1: u64 = 8_192;
pub const HINGE_RELIEF_LOCAL_INTERVAL_MODEL_ID_V1: &str =
    "linear_shared_hinge_local_open_interval_v1";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HingeReliefPolicyRecordV1 {
    pub edge: EdgeId,
    /// Full material removed normal to the hinge axis on each incident face.
    pub cutout_width_mm: f64,
    /// Included bevel angle in material cross-section, in degrees. V1 records
    /// it in binary64 but evaluates a rational conservative bound below
    /// instead of a platform libm tangent threshold.
    pub bevel_angle_degrees: f64,
    pub material_thickness_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HingeReliefLinearAngleScheduleV1 {
    pub edge: EdgeId,
    pub source_angle_degrees: f64,
    pub target_angle_degrees: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HingeReliefPolicyLimitsV1 {
    pub max_records: usize,
}

impl Default for HingeReliefPolicyLimitsV1 {
    fn default() -> Self {
        Self {
            max_records: MAX_HINGE_RELIEF_RECORDS_V1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum HingeReliefPolicyErrorV1 {
    #[error("hinge relief record limit is invalid")]
    InvalidLimit,
    #[error("hinge relief record limit exceeded")]
    ResourceLimit,
    #[error("hinge relief records are not in canonical edge order")]
    NonCanonicalOrder,
    #[error("hinge relief edge is duplicated")]
    DuplicateEdge,
    #[error("hinge relief edge is not present in the bound material graph")]
    UnknownHinge,
    #[error("hinge relief dimensions must be finite and positive")]
    InvalidDimension,
    #[error("hinge relief bevel angle must be finite and strictly between 0 and 180 degrees")]
    InvalidBevelAngle,
    #[error("hinge relief thickness does not exactly match the material thickness")]
    ThicknessMismatch,
    #[error("hinge relief cutout is too narrow for its thickness and bevel angle")]
    InsufficientCutout,
    #[error("hinge relief prerequisite is not bound to this graph or policy")]
    BindingMismatch,
    #[error("hinge relief schedule does not exactly cover the bound policy")]
    ScheduleBindingMismatch,
    #[error("hinge relief schedule angle is invalid")]
    InvalidSchedule,
}

/// Opaque, observation-only prerequisite for a future shared-hinge corridor
/// proof. It never authorizes a project mutation or admits a collision pair.
#[derive(Debug, Clone)]
pub struct NativeHingeReliefPrerequisiteV1 {
    graph: MaterialHingeGraphGeometry,
    material_thickness_bits: u64,
    records: Vec<HingeReliefPolicyRecordV1>,
}

#[derive(Debug, Clone)]
pub struct NativeHingeReliefLocalIntervalCertificateV1 {
    prerequisite: NativeHingeReliefPrerequisiteV1,
    schedules: Vec<HingeReliefLinearAngleScheduleV1>,
    exact_derivatives: Vec<BigRational>,
    exact_minimum_angles: Vec<BigRational>,
    exact_clearance_margins: Vec<BigRational>,
}

#[derive(Debug, PartialEq)]
struct ExactLocalScheduleProofV1 {
    derivatives: Vec<BigRational>,
    minimums: Vec<BigRational>,
    margins: Vec<BigRational>,
}

impl NativeHingeReliefLocalIntervalCertificateV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        HINGE_RELIEF_LOCAL_INTERVAL_MODEL_ID_V1
    }

    #[must_use]
    pub const fn authorizes_whole_path(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_shared_hinge_admission(&self) -> bool {
        false
    }

    #[must_use]
    pub fn schedule_count(&self) -> usize {
        self.schedules.len()
    }
}

pub fn certify_hinge_relief_local_intervals_v1(
    prerequisite: &NativeHingeReliefPrerequisiteV1,
    graph: &MaterialHingeGraphGeometry,
    material_thickness_mm: f64,
    records: &[HingeReliefPolicyRecordV1],
    schedules: &[HingeReliefLinearAngleScheduleV1],
    limits: HingeReliefPolicyLimitsV1,
) -> Result<NativeHingeReliefLocalIntervalCertificateV1, HingeReliefPolicyErrorV1> {
    revalidate_hinge_relief_prerequisite_v1(
        prerequisite,
        graph,
        material_thickness_mm,
        records,
        limits,
    )?;
    let exact = validate_local_schedules(records, schedules)?;
    Ok(NativeHingeReliefLocalIntervalCertificateV1 {
        prerequisite: prerequisite.clone(),
        schedules: schedules.to_vec(),
        exact_derivatives: exact.derivatives,
        exact_minimum_angles: exact.minimums,
        exact_clearance_margins: exact.margins,
    })
}

pub fn revalidate_hinge_relief_local_intervals_v1(
    certificate: &NativeHingeReliefLocalIntervalCertificateV1,
    prerequisite: &NativeHingeReliefPrerequisiteV1,
    graph: &MaterialHingeGraphGeometry,
    material_thickness_mm: f64,
    records: &[HingeReliefPolicyRecordV1],
    schedules: &[HingeReliefLinearAngleScheduleV1],
    limits: HingeReliefPolicyLimitsV1,
) -> Result<(), HingeReliefPolicyErrorV1> {
    revalidate_hinge_relief_prerequisite_v1(
        prerequisite,
        graph,
        material_thickness_mm,
        records,
        limits,
    )?;
    let exact = validate_local_schedules(records, schedules)?;
    if !certificate.prerequisite.graph.same_instance(graph)
        || certificate.prerequisite.material_thickness_bits != material_thickness_mm.to_bits()
        || certificate.prerequisite.records != records
        || certificate.schedules != schedules
        || certificate.exact_derivatives != exact.derivatives
        || certificate.exact_minimum_angles != exact.minimums
        || certificate.exact_clearance_margins != exact.margins
    {
        return Err(HingeReliefPolicyErrorV1::BindingMismatch);
    }
    Ok(())
}

fn validate_local_schedules(
    records: &[HingeReliefPolicyRecordV1],
    schedules: &[HingeReliefLinearAngleScheduleV1],
) -> Result<ExactLocalScheduleProofV1, HingeReliefPolicyErrorV1> {
    if records.len() != schedules.len() {
        return Err(HingeReliefPolicyErrorV1::ScheduleBindingMismatch);
    }
    let mut derivatives = Vec::new();
    let mut minimums = Vec::new();
    let mut margins = Vec::new();
    derivatives
        .try_reserve_exact(records.len())
        .map_err(|_| HingeReliefPolicyErrorV1::ResourceLimit)?;
    minimums
        .try_reserve_exact(records.len())
        .map_err(|_| HingeReliefPolicyErrorV1::ResourceLimit)?;
    margins
        .try_reserve_exact(records.len())
        .map_err(|_| HingeReliefPolicyErrorV1::ResourceLimit)?;
    for (record, schedule) in records.iter().zip(schedules) {
        if record.edge != schedule.edge {
            return Err(HingeReliefPolicyErrorV1::ScheduleBindingMismatch);
        }
        for angle in [schedule.source_angle_degrees, schedule.target_angle_degrees] {
            if !angle.is_finite() || !(0.0..=180.0).contains(&angle) || angle == 0.0 {
                return Err(HingeReliefPolicyErrorV1::InvalidSchedule);
            }
        }
        let source = BigRational::from_f64(schedule.source_angle_degrees)
            .ok_or(HingeReliefPolicyErrorV1::InvalidSchedule)?;
        let target = BigRational::from_f64(schedule.target_angle_degrees)
            .ok_or(HingeReliefPolicyErrorV1::InvalidSchedule)?;
        let derivative = &target - &source;
        let minimum = std::cmp::min(source, target);
        let width = BigRational::from_f64(record.cutout_width_mm)
            .ok_or(HingeReliefPolicyErrorV1::InvalidDimension)?;
        let thickness = BigRational::from_f64(record.material_thickness_mm)
            .ok_or(HingeReliefPolicyErrorV1::InvalidDimension)?;
        let margin = width * &minimum - thickness * BigRational::from_integer(60.into());
        let exact_bits = rational_bits(&derivative)
            .checked_add(rational_bits(&minimum))
            .and_then(|bits| bits.checked_add(rational_bits(&margin)))
            .ok_or(HingeReliefPolicyErrorV1::ResourceLimit)?;
        if exact_bits > MAX_HINGE_RELIEF_EXACT_BITS_PER_RECORD_V1 {
            return Err(HingeReliefPolicyErrorV1::ResourceLimit);
        }
        if margin < BigRational::from_integer(0.into()) {
            return Err(HingeReliefPolicyErrorV1::InsufficientCutout);
        }
        derivatives.push(derivative);
        minimums.push(minimum);
        margins.push(margin);
    }
    Ok(ExactLocalScheduleProofV1 {
        derivatives,
        minimums,
        margins,
    })
}

fn rational_bits(value: &BigRational) -> u64 {
    value.numer().bits().saturating_add(value.denom().bits())
}

impl NativeHingeReliefPrerequisiteV1 {
    #[must_use]
    pub const fn model_id(&self) -> &'static str {
        HINGE_RELIEF_POLICY_MODEL_ID_V1
    }

    #[must_use]
    pub const fn authorizes_project_mutation(&self) -> bool {
        false
    }

    #[must_use]
    pub const fn authorizes_shared_hinge_admission(&self) -> bool {
        false
    }

    #[must_use]
    pub fn records(&self) -> &[HingeReliefPolicyRecordV1] {
        &self.records
    }
}

pub fn prepare_hinge_relief_prerequisite_v1(
    graph: &MaterialHingeGraphGeometry,
    material_thickness_mm: f64,
    records: &[HingeReliefPolicyRecordV1],
    limits: HingeReliefPolicyLimitsV1,
) -> Result<NativeHingeReliefPrerequisiteV1, HingeReliefPolicyErrorV1> {
    preflight_graph_work(graph, records, limits)?;
    let hinge_edges = graph.hinges().iter().map(|hinge| hinge.edge());
    validate_policy(hinge_edges, material_thickness_mm, records, limits)?;
    Ok(NativeHingeReliefPrerequisiteV1 {
        graph: graph.clone(),
        material_thickness_bits: material_thickness_mm.to_bits(),
        records: records.to_vec(),
    })
}

pub fn revalidate_hinge_relief_prerequisite_v1(
    prerequisite: &NativeHingeReliefPrerequisiteV1,
    graph: &MaterialHingeGraphGeometry,
    material_thickness_mm: f64,
    records: &[HingeReliefPolicyRecordV1],
    limits: HingeReliefPolicyLimitsV1,
) -> Result<(), HingeReliefPolicyErrorV1> {
    preflight_graph_work(graph, records, limits)?;
    validate_policy(
        graph.hinges().iter().map(|hinge| hinge.edge()),
        material_thickness_mm,
        records,
        limits,
    )?;
    if !prerequisite.graph.same_instance(graph)
        || prerequisite.material_thickness_bits != material_thickness_mm.to_bits()
        || prerequisite.records != records
    {
        return Err(HingeReliefPolicyErrorV1::BindingMismatch);
    }
    Ok(())
}

fn preflight_graph_work(
    graph: &MaterialHingeGraphGeometry,
    records: &[HingeReliefPolicyRecordV1],
    limits: HingeReliefPolicyLimitsV1,
) -> Result<(), HingeReliefPolicyErrorV1> {
    if limits.max_records > MAX_HINGE_RELIEF_RECORDS_V1 {
        return Err(HingeReliefPolicyErrorV1::InvalidLimit);
    }
    if graph.hinges().len() > limits.max_records || records.len() > limits.max_records {
        return Err(HingeReliefPolicyErrorV1::ResourceLimit);
    }
    Ok(())
}

fn validate_policy(
    hinge_edges: impl IntoIterator<Item = EdgeId>,
    material_thickness_mm: f64,
    records: &[HingeReliefPolicyRecordV1],
    limits: HingeReliefPolicyLimitsV1,
) -> Result<(), HingeReliefPolicyErrorV1> {
    if limits.max_records > MAX_HINGE_RELIEF_RECORDS_V1 {
        return Err(HingeReliefPolicyErrorV1::InvalidLimit);
    }
    if records.len() > limits.max_records {
        return Err(HingeReliefPolicyErrorV1::ResourceLimit);
    }
    if !material_thickness_mm.is_finite() || material_thickness_mm <= 0.0 {
        return Err(HingeReliefPolicyErrorV1::InvalidDimension);
    }
    let hinges = hinge_edges.into_iter().collect::<HashSet<_>>();
    for pair in records.windows(2) {
        let order = pair[0]
            .edge
            .canonical_bytes()
            .cmp(&pair[1].edge.canonical_bytes());
        if order.is_gt() {
            return Err(HingeReliefPolicyErrorV1::NonCanonicalOrder);
        }
        if order.is_eq() {
            return Err(HingeReliefPolicyErrorV1::DuplicateEdge);
        }
    }
    for record in records {
        if !hinges.contains(&record.edge) {
            return Err(HingeReliefPolicyErrorV1::UnknownHinge);
        }
        if !record.cutout_width_mm.is_finite()
            || record.cutout_width_mm <= 0.0
            || !record.material_thickness_mm.is_finite()
            || record.material_thickness_mm <= 0.0
        {
            return Err(HingeReliefPolicyErrorV1::InvalidDimension);
        }
        if record.material_thickness_mm.to_bits() != material_thickness_mm.to_bits() {
            return Err(HingeReliefPolicyErrorV1::ThicknessMismatch);
        }
        if !record.bevel_angle_degrees.is_finite()
            || !(0.0..180.0).contains(&record.bevel_angle_degrees)
            || record.bevel_angle_degrees == 0.0
        {
            return Err(HingeReliefPolicyErrorV1::InvalidBevelAngle);
        }
        // Let x=theta*pi/360. On (0, pi/2), tan(x)>=x and pi>=3, hence
        // t/(2*tan(x)) <= 60*t/theta. Proving w*theta>=60*t using exact
        // rationals is therefore conservative and platform deterministic.
        let width = BigRational::from_f64(record.cutout_width_mm)
            .ok_or(HingeReliefPolicyErrorV1::InvalidDimension)?;
        let angle = BigRational::from_f64(record.bevel_angle_degrees)
            .ok_or(HingeReliefPolicyErrorV1::InvalidBevelAngle)?;
        let thickness = BigRational::from_f64(record.material_thickness_mm)
            .ok_or(HingeReliefPolicyErrorV1::InvalidDimension)?;
        let left = width * angle;
        let right = thickness * BigRational::from_integer(60.into());
        let exact_bits = left
            .numer()
            .bits()
            .checked_add(left.denom().bits())
            .and_then(|bits| bits.checked_add(right.numer().bits()))
            .and_then(|bits| bits.checked_add(right.denom().bits()))
            .ok_or(HingeReliefPolicyErrorV1::ResourceLimit)?;
        if exact_bits > MAX_HINGE_RELIEF_EXACT_BITS_PER_RECORD_V1 {
            return Err(HingeReliefPolicyErrorV1::ResourceLimit);
        }
        if left < right {
            return Err(HingeReliefPolicyErrorV1::InsufficientCutout);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted_edges(count: usize) -> Vec<EdgeId> {
        let mut edges = (0..count).map(|_| EdgeId::new()).collect::<Vec<_>>();
        edges.sort_unstable_by_key(EdgeId::canonical_bytes);
        edges
    }

    fn records(edges: &[EdgeId]) -> Vec<HingeReliefPolicyRecordV1> {
        edges
            .iter()
            .map(|&edge| HingeReliefPolicyRecordV1 {
                edge,
                cutout_width_mm: 0.1,
                bevel_angle_degrees: 90.0,
                material_thickness_mm: 0.1,
            })
            .collect()
    }

    #[test]
    fn policy_validation_is_bounded_canonical_and_complete_at_four_eight_sixteen() {
        for count in [4, 8, 16] {
            let edges = sorted_edges(count);
            let records = records(&edges);
            validate_policy(
                edges.clone(),
                0.1,
                &records,
                HingeReliefPolicyLimitsV1::default(),
            )
            .unwrap();
        }
        let edges = sorted_edges(MAX_HINGE_RELIEF_RECORDS_V1 + 1);
        assert_eq!(
            validate_policy(
                edges.clone(),
                0.1,
                &records(&edges),
                HingeReliefPolicyLimitsV1::default(),
            ),
            Err(HingeReliefPolicyErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn policy_rejects_tamper_and_accepts_empty_default_compatibility() {
        validate_policy(
            std::iter::empty(),
            0.1,
            &[],
            HingeReliefPolicyLimitsV1::default(),
        )
        .unwrap();
        let edges = sorted_edges(2);
        let mut input = records(&edges);
        input.reverse();
        assert_eq!(
            validate_policy(
                edges.clone(),
                0.1,
                &input,
                HingeReliefPolicyLimitsV1::default()
            ),
            Err(HingeReliefPolicyErrorV1::NonCanonicalOrder)
        );
        let mut input = records(&edges);
        input[0].material_thickness_mm = 0.2;
        assert_eq!(
            validate_policy(edges, 0.1, &input, HingeReliefPolicyLimitsV1::default()),
            Err(HingeReliefPolicyErrorV1::ThicknessMismatch)
        );
    }

    #[test]
    fn conservative_cutout_boundary_accepts_equal_and_rejects_the_previous_float() {
        let edges = sorted_edges(1);
        let mut input = records(&edges);
        input[0].bevel_angle_degrees = 60.0;
        validate_policy(
            edges.clone(),
            0.1,
            &input,
            HingeReliefPolicyLimitsV1::default(),
        )
        .unwrap();
        input[0].cutout_width_mm = f64::from_bits(0.1_f64.to_bits() - 1);
        assert_eq!(
            validate_policy(
                edges.clone(),
                0.1,
                &input,
                HingeReliefPolicyLimitsV1::default()
            ),
            Err(HingeReliefPolicyErrorV1::InsufficientCutout)
        );
        input[0].cutout_width_mm = f64::from_bits(0.1_f64.to_bits() + 1);
        validate_policy(
            edges.clone(),
            0.1,
            &input,
            HingeReliefPolicyLimitsV1::default(),
        )
        .unwrap();

        input[0].bevel_angle_degrees = 10.0;
        input[0].cutout_width_mm = 0.1;
        assert_eq!(
            validate_policy(
                edges.clone(),
                0.1,
                &input,
                HingeReliefPolicyLimitsV1::default()
            ),
            Err(HingeReliefPolicyErrorV1::InsufficientCutout)
        );
        for angle in [1.0, 10.0, 90.0, 179.0] {
            input[0].bevel_angle_degrees = angle;
            input[0].cutout_width_mm = 7.0;
            validate_policy(
                edges.clone(),
                0.1,
                &input,
                HingeReliefPolicyLimitsV1::default(),
            )
            .unwrap();
        }
        input[0].bevel_angle_degrees = 1.0;
        input[0].cutout_width_mm = 6.0;
        assert_eq!(
            validate_policy(
                edges.clone(),
                0.1,
                &input,
                HingeReliefPolicyLimitsV1::default()
            ),
            Err(HingeReliefPolicyErrorV1::InsufficientCutout)
        );
        input[0].cutout_width_mm = f64::from_bits(6.0_f64.to_bits() + 1);
        validate_policy(edges, 0.1, &input, HingeReliefPolicyLimitsV1::default()).unwrap();
    }

    #[test]
    fn linear_local_intervals_are_exact_at_four_eight_sixteen_and_reject_tamper() {
        for count in [4, 8, 16] {
            let edges = sorted_edges(count);
            let records = records(&edges);
            let schedules = edges
                .iter()
                .map(|&edge| HingeReliefLinearAngleScheduleV1 {
                    edge,
                    source_angle_degrees: 90.0,
                    target_angle_degrees: 120.0,
                })
                .collect::<Vec<_>>();
            let exact = validate_local_schedules(&records, &schedules).unwrap();
            assert_eq!(exact.derivatives.len(), count);
            assert_eq!(exact.minimums.len(), count);
            assert_eq!(exact.margins.len(), count);
            let zero = BigRational::from_integer(0.into());
            assert!(exact.margins.iter().all(|margin| margin >= &zero));

            let mut tampered = schedules.clone();
            tampered[0].edge = EdgeId::new();
            assert_eq!(
                validate_local_schedules(&records, &tampered),
                Err(HingeReliefPolicyErrorV1::ScheduleBindingMismatch)
            );
        }

        let edges = sorted_edges(1);
        let mut records = records(&edges);
        let mut schedules = vec![HingeReliefLinearAngleScheduleV1 {
            edge: edges[0],
            source_angle_degrees: 60.0,
            target_angle_degrees: 120.0,
        }];
        validate_local_schedules(&records, &schedules).unwrap();
        records[0].cutout_width_mm = f64::from_bits(0.1_f64.to_bits() - 1);
        assert_eq!(
            validate_local_schedules(&records, &schedules),
            Err(HingeReliefPolicyErrorV1::InsufficientCutout)
        );
        records[0].cutout_width_mm = 0.1;
        for invalid in [0.0, -1.0, 181.0, f64::NAN, f64::INFINITY] {
            schedules[0].source_angle_degrees = invalid;
            assert_eq!(
                validate_local_schedules(&records, &schedules),
                Err(HingeReliefPolicyErrorV1::InvalidSchedule)
            );
        }
    }
}
