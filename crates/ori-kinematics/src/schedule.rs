use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
use ori_domain::{EdgeId, FaceId};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CanonicalHingeAngles, HingeAngle, MaterialHingeGraphAudit, MaterialHingeGraphGeometry,
    OutwardIntervalV1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RationalCoefficientV1 {
    pub numerator: i64,
    pub denominator: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HalfAngleDomainV1 {
    angle_degrees: [f64; 2],
    half_angle_tangent: OutwardIntervalV1,
}

impl HalfAngleDomainV1 {
    pub fn prepare(angle_degrees: [f64; 2]) -> Result<Self, CycleSchedulePrepareErrorV1> {
        if !angle_degrees[0].is_finite()
            || !angle_degrees[1].is_finite()
            || angle_degrees[0] >= angle_degrees[1]
            || angle_degrees[0] <= -180.0
            || angle_degrees[1] >= 180.0
        {
            return Err(CycleSchedulePrepareErrorV1::InvalidInput);
        }
        let lower = libm::tan(angle_degrees[0] * core::f64::consts::PI / 360.0);
        let upper = libm::tan(angle_degrees[1] * core::f64::consts::PI / 360.0);
        let lower = OutwardIntervalV1::from_rounded(lower)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        let upper = OutwardIntervalV1::from_rounded(upper)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        let half_angle_tangent = OutwardIntervalV1::new(lower.lower(), upper.upper())
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        Ok(Self {
            angle_degrees,
            half_angle_tangent,
        })
    }

    #[must_use]
    pub const fn angle_degrees(&self) -> [f64; 2] {
        self.angle_degrees
    }

    #[must_use]
    pub const fn half_angle_tangent(&self) -> OutwardIntervalV1 {
        self.half_angle_tangent
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoleFreeBernsteinCertificateV1 {
    degree: usize,
    positive: bool,
    coefficients: Vec<BigRational>,
}

impl PoleFreeBernsteinCertificateV1 {
    fn range_interval(&self) -> Result<OutwardIntervalV1, CycleSchedulePrepareErrorV1> {
        let lower = self
            .coefficients
            .iter()
            .min()
            .and_then(|value| value.to_f64())
            .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
        let upper = self
            .coefficients
            .iter()
            .max()
            .and_then(|value| value.to_f64())
            .ok_or(CycleSchedulePrepareErrorV1::InvalidInput)?;
        let lower = OutwardIntervalV1::from_rounded(lower)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        let upper = OutwardIntervalV1::from_rounded(upper)
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)?;
        OutwardIntervalV1::new(lower.lower(), upper.upper())
            .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)
    }
}

pub fn evaluate_pole_free_rational_interval_v1(
    numerator: &PoleFreeBernsteinCertificateV1,
    denominator: &PoleFreeBernsteinCertificateV1,
    max_work: usize,
) -> Result<OutwardIntervalV1, CycleSchedulePrepareErrorV1> {
    let work = numerator
        .coefficients
        .len()
        .checked_add(denominator.coefficients.len())
        .ok_or(CycleSchedulePrepareErrorV1::ResourceLimit)?;
    if work > max_work {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
    }
    let numerator = numerator.range_interval()?;
    let denominator = denominator.range_interval()?;
    if denominator.lower() <= 0.0 && denominator.upper() >= 0.0 {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput);
    }
    numerator
        .div(denominator)
        .map_err(|_| CycleSchedulePrepareErrorV1::InvalidInput)
}

pub fn prepare_pole_free_bernstein_certificate_v1(
    power_coefficients: &[RationalCoefficientV1],
    max_degree: usize,
    max_coefficient_bits: u32,
    max_work: usize,
) -> Result<PoleFreeBernsteinCertificateV1, CycleSchedulePrepareErrorV1> {
    if power_coefficients.is_empty()
        || power_coefficients.len() > max_degree.saturating_add(1)
        || power_coefficients
            .len()
            .saturating_mul(power_coefficients.len())
            > max_work
    {
        return Err(CycleSchedulePrepareErrorV1::ResourceLimit);
    }
    let degree = power_coefficients.len() - 1;
    let power = power_coefficients
        .iter()
        .map(|value| {
            if value.denominator == 0
                || value.numerator.unsigned_abs().checked_ilog2().unwrap_or(0) + 1
                    > max_coefficient_bits
                || value.denominator.checked_ilog2().unwrap_or(0) + 1 > max_coefficient_bits
            {
                return Err(CycleSchedulePrepareErrorV1::InvalidInput);
            }
            Ok(BigRational::new(
                BigInt::from(value.numerator),
                BigInt::from(value.denominator),
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut coefficients = Vec::with_capacity(degree + 1);
    for i in 0..=degree {
        let mut value = BigRational::zero();
        for (k, coefficient) in power.iter().enumerate().take(i + 1) {
            value += coefficient
                * BigRational::new(
                    BigInt::from(binomial(i, k)),
                    BigInt::from(binomial(degree, k)),
                );
        }
        coefficients.push(value);
    }
    let positive = coefficients.iter().all(|value| value.is_positive());
    let negative = coefficients.iter().all(|value| value.is_negative());
    if !positive && !negative {
        return Err(CycleSchedulePrepareErrorV1::InvalidInput);
    }
    Ok(PoleFreeBernsteinCertificateV1 {
        degree,
        positive,
        coefficients,
    })
}

fn binomial(n: usize, k: usize) -> u128 {
    let k = k.min(n - k);
    (0..k).fold(1_u128, |value, i| value * (n - i) as u128 / (i + 1) as u128)
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
        let mut expected = geometry
            .hinges()
            .iter()
            .map(|hinge| hinge.edge())
            .collect::<Vec<_>>();
        expected.sort_unstable_by_key(EdgeId::canonical_bytes);
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

#[cfg(test)]
mod tests {
    use ori_domain::{EdgeId, FaceId, ProjectId};
    use ori_topology::{
        BoundaryWalk, Face, FaceAdjacency, FaceKey, FoldAssignment, TopologySnapshot,
    };

    use super::*;
    use crate::{Point3, TreeHinge, TreeKinematicsLimits};

    fn fixture() -> (
        MaterialHingeGraphGeometry,
        MaterialHingeGraphAudit,
        FaceId,
        Vec<EdgeId>,
    ) {
        let ns = ProjectId::new();
        let faces = [b"a", b"b", b"c"].map(|name| FaceId::derive_v5(ns, name));
        let edges = [b"ab", b"bc", b"ca"].map(|name| EdgeId::derive_v5(ns, name));
        let topology = TopologySnapshot {
            source_revision: 1,
            faces: faces
                .iter()
                .map(|id| Face {
                    id: *id,
                    key: FaceKey(id.canonical_bytes().repeat(2).try_into().unwrap()),
                    outer: BoundaryWalk {
                        half_edges: Vec::new(),
                        signed_double_area: 1.0,
                    },
                    holes: Vec::new(),
                    seams: Vec::new(),
                    area: 0.5,
                })
                .collect(),
            edge_incidence: Vec::new(),
            hinge_adjacency: (0..3)
                .map(|i| FaceAdjacency {
                    edge: edges[i],
                    first: faces[i],
                    second: faces[(i + 1) % 3],
                    assignment: FoldAssignment::Mountain,
                })
                .collect(),
            material_components: Vec::new(),
        };
        let audit =
            MaterialHingeGraphAudit::prepare(&topology, TreeKinematicsLimits::default()).unwrap();
        let start = Point3::new(0.0, 0.0, 0.0).unwrap();
        let end = Point3::new(1.0, 0.0, 0.0).unwrap();
        let hinges = (0..3)
            .map(|i| {
                TreeHinge::new_for_test(
                    edges[i],
                    FoldAssignment::Mountain,
                    faces[i],
                    faces[(i + 1) % 3],
                    start,
                    end,
                    end,
                )
            })
            .collect();
        (
            MaterialHingeGraphGeometry::new_for_test(faces.to_vec(), hinges),
            audit,
            faces[0],
            edges.to_vec(),
        )
    }

    fn entries(edges: &[EdgeId]) -> Vec<CycleScheduleEntryInputV1> {
        let mut entries = edges
            .iter()
            .map(|edge| CycleScheduleEntryInputV1 {
                edge: *edge,
                initial_angle_degrees_bits: 90.0_f64.to_bits(),
                chebyshev_coefficients: vec![
                    RationalCoefficientV1 {
                        numerator: 0,
                        denominator: 1,
                    },
                    RationalCoefficientV1 {
                        numerator: 10,
                        denominator: 1,
                    },
                ],
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by_key(|entry| entry.edge.canonical_bytes());
        entries
    }

    #[test]
    fn canonical_schedule_evaluates_and_bounds_derivative() {
        let (geometry, audit, fixed, edges) = fixture();
        let schedule = CanonicalCycleScheduleV1::prepare(
            &geometry,
            &audit,
            fixed,
            [0.0, 1.0],
            entries(&edges),
            CycleScheduleLimitsV1::default(),
        )
        .unwrap();
        assert!(schedule.matches_binding(&geometry, &audit, fixed));
        let forged_fixed = audit
            .faces()
            .iter()
            .copied()
            .find(|face| *face != fixed)
            .unwrap();
        assert!(!schedule.matches_binding(&geometry, &audit, forged_fixed));
        assert_eq!(
            schedule.evaluate(0.5).unwrap().as_slice()[0].angle_degrees(),
            90.0
        );
        assert_eq!(schedule.derivative_bound(edges[0]), Some(20.0));
        assert!(schedule.evaluate(-0.1).is_none());
    }

    #[test]
    fn malformed_order_coefficients_and_limits_fail_closed() {
        let (geometry, audit, fixed, edges) = fixture();
        let limits = CycleScheduleLimitsV1::default();
        let mut reversed = entries(&edges);
        reversed.reverse();
        assert_eq!(
            CanonicalCycleScheduleV1::prepare(
                &geometry,
                &audit,
                fixed,
                [0.0, 1.0],
                reversed,
                limits
            ),
            Err(CycleSchedulePrepareErrorV1::NonCanonical)
        );
        let mut bad = entries(&edges);
        bad[0].chebyshev_coefficients[0].denominator = 0;
        assert_eq!(
            CanonicalCycleScheduleV1::prepare(&geometry, &audit, fixed, [0.0, 1.0], bad, limits),
            Err(CycleSchedulePrepareErrorV1::InvalidInput)
        );
        let mut excessive = entries(&edges);
        excessive[0].chebyshev_coefficients.resize(
            limits.max_degree + 2,
            RationalCoefficientV1 {
                numerator: 0,
                denominator: 1,
            },
        );
        assert_eq!(
            CanonicalCycleScheduleV1::prepare(
                &geometry,
                &audit,
                fixed,
                [0.0, 1.0],
                excessive,
                limits
            ),
            Err(CycleSchedulePrepareErrorV1::ResourceLimit)
        );
        let mut wide = entries(&edges);
        wide[0].chebyshev_coefficients[0].numerator = 1_i64 << 54;
        assert_eq!(
            CanonicalCycleScheduleV1::prepare(&geometry, &audit, fixed, [0.0, 1.0], wide, limits),
            Err(CycleSchedulePrepareErrorV1::InvalidInput)
        );
        let mut out_of_range = entries(&edges);
        out_of_range[0].chebyshev_coefficients[1].numerator = 91;
        assert_eq!(
            CanonicalCycleScheduleV1::prepare(
                &geometry,
                &audit,
                fixed,
                [0.0, 1.0],
                out_of_range,
                limits,
            ),
            Err(CycleSchedulePrepareErrorV1::AngleRange)
        );
        assert_eq!(
            CanonicalCycleScheduleV1::prepare(
                &geometry,
                &audit,
                fixed,
                [0.0, 1.0],
                entries(&edges),
                CycleScheduleLimitsV1 {
                    max_work: 1,
                    ..limits
                },
            ),
            Err(CycleSchedulePrepareErrorV1::ResourceLimit)
        );
    }

    #[test]
    fn exact_bernstein_certificate_proves_only_strict_single_sign_denominators() {
        let positive = prepare_pole_free_bernstein_certificate_v1(
            &[
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
            ],
            4,
            8,
            16,
        )
        .unwrap();
        assert!(positive.positive);
        assert_eq!(positive.degree, 1);
        assert!(
            positive
                .coefficients
                .iter()
                .all(|value| value.is_positive())
        );
        let denominator = prepare_pole_free_bernstein_certificate_v1(
            &[RationalCoefficientV1 {
                numerator: 2,
                denominator: 1,
            }],
            4,
            8,
            16,
        )
        .unwrap();
        let quotient =
            evaluate_pole_free_rational_interval_v1(&positive, &denominator, 16).unwrap();
        assert!(quotient.lower() <= 0.5);
        assert!(quotient.upper() >= 1.0);
        assert_eq!(
            evaluate_pole_free_rational_interval_v1(&positive, &denominator, 1),
            Err(CycleSchedulePrepareErrorV1::ResourceLimit)
        );
        let near_zero = prepare_pole_free_bernstein_certificate_v1(
            &[RationalCoefficientV1 {
                numerator: 1,
                denominator: 1_u64 << 50,
            }],
            4,
            53,
            16,
        )
        .unwrap();
        let large =
            evaluate_pole_free_rational_interval_v1(&positive, &near_zero, 16).unwrap();
        assert!(large.upper().is_finite());
        assert!(large.lower() > 0.0);
        for invalid in [
            vec![
                RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                },
                RationalCoefficientV1 {
                    numerator: -2,
                    denominator: 1,
                },
            ],
            vec![RationalCoefficientV1 {
                numerator: 1,
                denominator: 0,
            }],
        ] {
            assert!(prepare_pole_free_bernstein_certificate_v1(&invalid, 4, 8, 16).is_err());
        }
        assert!(
            prepare_pole_free_bernstein_certificate_v1(
                &[RationalCoefficientV1 {
                    numerator: 256,
                    denominator: 1,
                }],
                4,
                8,
                16,
            )
            .is_err()
        );
        assert!(
            prepare_pole_free_bernstein_certificate_v1(
                &[RationalCoefficientV1 {
                    numerator: 1,
                    denominator: 1,
                }; 5],
                3,
                8,
                16,
            )
            .is_err()
        );
    }

    #[test]
    fn half_angle_domain_separates_tangent_poles_and_encloses_known_angles() {
        let domain = HalfAngleDomainV1::prepare([-90.0, 90.0]).unwrap();
        assert_eq!(domain.angle_degrees(), [-90.0, 90.0]);
        let tangent = domain.half_angle_tangent();
        assert!(tangent.lower() <= -1.0);
        assert!(tangent.upper() >= 1.0);
        for invalid in [
            [-180.0, 0.0],
            [0.0, 180.0],
            [1.0, 1.0],
            [f64::NAN, 1.0],
        ] {
            assert_eq!(
                HalfAngleDomainV1::prepare(invalid),
                Err(CycleSchedulePrepareErrorV1::InvalidInput)
            );
        }
        let near_poles = HalfAngleDomainV1::prepare([-179.0, 179.0]).unwrap();
        assert!(near_poles.half_angle_tangent().lower() < -100.0);
        assert!(near_poles.half_angle_tangent().upper() > 100.0);
    }
}
