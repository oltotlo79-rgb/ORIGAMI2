use ori_domain::{EdgeId, FaceId};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CanonicalHingeAngles, HingeAngle, MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RationalCoefficientV1 {
    pub numerator: i64,
    pub denominator: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleScheduleEntryInputV1 {
    pub edge: EdgeId,
    pub initial_angle_degrees_bits: u64,
    pub chebyshev_coefficients: Vec<RationalCoefficientV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CycleScheduleLimitsV1 {
    pub max_hinges: usize,
    pub max_degree: usize,
    pub max_coefficient_bits: u32,
    pub max_work: usize,
}

impl Default for CycleScheduleLimitsV1 {
    fn default() -> Self {
        Self {
            max_hinges: 64,
            max_degree: 8,
            max_coefficient_bits: 53,
            max_work: 576,
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CycleSchedulePrepareErrorV1 {
    #[error("cycle schedule input is malformed")]
    InvalidInput,
    #[error("cycle schedule order or carrier set is not canonical")]
    NonCanonical,
    #[error("cycle schedule exceeds its work limits")]
    ResourceLimit,
    #[error("cycle schedule leaves the physical hinge-angle range")]
    AngleRange,
}

#[derive(Debug, Clone, PartialEq)]
struct Entry {
    edge: EdgeId,
    initial: f64,
    coefficients: Vec<f64>,
    derivative_bound: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalCycleScheduleV1 {
    binding_fingerprint: [u8; 32],
    fixed_face: FaceId,
    domain: [f64; 2],
    entries: Vec<Entry>,
}

impl CanonicalCycleScheduleV1 {
    pub fn prepare(
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
        domain: [f64; 2],
        entries: Vec<CycleScheduleEntryInputV1>,
        limits: CycleScheduleLimitsV1,
    ) -> Result<Self, CycleSchedulePrepareErrorV1> {
        if !domain[0].is_finite()
            || !domain[1].is_finite()
            || domain[0] >= domain[1]
            || entries.is_empty()
            || entries.len() > limits.max_hinges
            || entries.len() != geometry.hinges().len()
            || !audit.faces().contains(&fixed_face)
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let work = entries
            .iter()
            .try_fold(0usize, |sum, entry| {
                sum.checked_add(entry.chebyshev_coefficients.len())
            })
            .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
        if work > limits.max_work {
            return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
        }
        let expected = geometry
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        if entries.iter().map(|entry| entry.edge).collect::<Vec<_>>() != expected {
            return Err(CycleSchedulePrepareErrorV1::NonCanonical);
        }
        let width = domain[1] - domain[0];
        let mut prepared = Vec::with_capacity(entries.len());
        for input in entries {
            if input.chebyshev_coefficients.len() > limits.max_degree + 1 {
                return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
            }
            let initial = f64::from_bits(input.initial_angle_degrees_bits);
            if !initial.is_finite() || !(0.0..=180.0).contains(&initial) {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput);
            }
            let mut coefficients = Vec::with_capacity(input.chebyshev_coefficients.len());
            for coefficient in input.chebyshev_coefficients {
                if coefficient.denominator == 0
                    || coefficient
                        .numerator
                        .unsigned_abs()
                        .checked_ilog2()
                        .unwrap_or(0)
                        .saturating_add(1)
                        > limits.max_coefficient_bits
                    || coefficient
                        .denominator
                        .checked_ilog2()
                        .unwrap_or(0)
                        .saturating_add(1)
                        > limits.max_coefficient_bits
                {
                    return Err(CycleSchedulePrepareErrorV1::InvalidInput);
                }
                coefficients.push(coefficient.numerator as f64 / coefficient.denominator as f64);
            }
            let excursion = coefficients.iter().map(|value| value.abs()).sum::<f64>();
            if initial - excursion < 0.0 || initial + excursion > 180.0 {
                return Err(CycleSchedulePrepareErrorV1::AngleRange);
            }
            let derivative_bound = coefficients
                .iter()
                .enumerate()
                .map(|(degree, value)| 2.0 * (degree * degree) as f64 * value.abs() / width)
                .sum();
            prepared.push(Entry {
                edge: input.edge,
                initial,
                coefficients,
                derivative_bound,
            });
        }
        Ok(Self {
            binding_fingerprint: binding_fingerprint(geometry, audit, fixed_face),
            fixed_face,
            domain,
            entries: prepared,
        })
    }

    pub fn evaluate(&self, parameter: f64) -> Option<CanonicalHingeAngles> {
        if !parameter.is_finite() || parameter < self.domain[0] || parameter > self.domain[1] {
            return None;
        }
        let x =
            (2.0 * parameter - self.domain[0] - self.domain[1]) / (self.domain[1] - self.domain[0]);
        self.entries
            .iter()
            .map(|entry| {
                let mut b1 = 0.0;
                let mut b2 = 0.0;
                for coefficient in entry.coefficients.iter().rev() {
                    let b0 = 2.0 * x * b1 - b2 + coefficient;
                    b2 = b1;
                    b1 = b0;
                }
                HingeAngle::new(entry.edge, entry.initial + b1 - x * b2).ok()
            })
            .collect::<Option<Vec<_>>>()
            .and_then(|angles| CanonicalHingeAngles::new(angles).ok())
    }

    #[must_use]
    pub fn derivative_bound(&self, edge: EdgeId) -> Option<f64> {
        self.entries
            .iter()
            .find(|entry| entry.edge == edge)
            .map(|entry| entry.derivative_bound)
    }

    #[must_use]
    pub fn matches_binding(
        &self,
        geometry: &MaterialHingeGraphGeometry,
        audit: &MaterialHingeGraphAudit,
        fixed_face: FaceId,
    ) -> bool {
        self.fixed_face == fixed_face
            && self.binding_fingerprint == binding_fingerprint(geometry, audit, fixed_face)
    }
}

fn binding_fingerprint(
    geometry: &MaterialHingeGraphGeometry,
    audit: &MaterialHingeGraphAudit,
    fixed_face: FaceId,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(fixed_face.canonical_bytes());
    for face in audit.faces() {
        hash.update(face.canonical_bytes());
    }
    for edge in audit.spanning_hinges().iter().chain(audit.closure_hinges()) {
        hash.update(edge.canonical_bytes());
    }
    for hinge in geometry.hinges() {
        hash.update(hinge.edge().canonical_bytes());
        hash.update(hinge.left_face().canonical_bytes());
        hash.update(hinge.right_face().canonical_bytes());
        for value in [
            hinge.start().x(),
            hinge.start().y(),
            hinge.start().z(),
            hinge.end().x(),
            hinge.end().y(),
            hinge.end().z(),
        ] {
            hash.update(value.to_bits().to_be_bytes());
        }
    }
    hash.finalize().into()
}
