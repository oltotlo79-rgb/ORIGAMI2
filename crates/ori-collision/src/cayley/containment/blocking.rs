//! Blocking-only robust-transversal primitive in actual millimetres.
//!
//! This checkpoint deliberately accepts only intervals expressed in the
//! rational Cayley tree's actual-mm coordinate system.  The zero-thickness
//! classifier's binary64 common-unit coordinates are scaled by `2^1074` and
//! must never enter this API.  A later authority bridge must also bind a
//! measured affine envelope to the canonical exact pose that it measured
//! before this primitive can consume either object.
//! That future bridge must first obtain an exact-geometry `Transversal`
//! classification for the same authenticated pair, must not call this
//! zero-thickness primitive for positive paper thickness, and must keep
//! coplanar/180-degree cases unresolved.
//!
//! The only positive result is a transversal penetration.  Touching,
//! separation, coplanarity, shared-hinge contact, numeric uncertainty and
//! every resource failure all collapse to `Unresolved`.  Consequently this
//! type cannot widen any collision-free set or issue a public geometry proof.

use std::cmp::Ordering;

use num_rational::BigRational;
use num_traits::{One, Signed};

use super::super::{CayleyError, CayleyLimits, CayleyStage, CayleyWork, WorkMeter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockingOnlyDecision {
    ProvenPenetrating,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PairTopology {
    NoSharedFeature,
    SharedVertex {
        first_vertex: usize,
        second_vertex: usize,
    },
    SharedHinge,
    SameFace,
}

/// Dimension-neutral closed rational interval used by the robust predicate.
///
/// Coordinates acquire the actual-mm contract only when stored in
/// [`ActualMillimetrePointInterval`]. Determinants and interpolation
/// parameters deliberately reuse this scalar container without claiming that
/// their dimensions are millimetres.
#[derive(Debug, Clone)]
struct ClosedRationalInterval {
    lower: BigRational,
    upper: BigRational,
}

impl ClosedRationalInterval {
    #[cfg(test)]
    fn point(value: BigRational) -> Self {
        Self {
            lower: value.clone(),
            upper: value,
        }
    }

    #[cfg(test)]
    fn inflated(value: BigRational, radius: BigRational) -> Self {
        assert!(!radius.is_negative());
        Self {
            lower: &value - &radius,
            upper: value + radius,
        }
    }

    fn add(&self, other: &Self, meter: &mut WorkMeter<'_>) -> Result<Self, CayleyError> {
        Self::ordered(
            meter.add_rational(&self.lower, &other.lower, CayleyStage::Containment)?,
            meter.add_rational(&self.upper, &other.upper, CayleyStage::Containment)?,
            meter,
        )
    }

    fn subtract(&self, other: &Self, meter: &mut WorkMeter<'_>) -> Result<Self, CayleyError> {
        Self::ordered(
            meter.subtract_rational(&self.lower, &other.upper, CayleyStage::Containment)?,
            meter.subtract_rational(&self.upper, &other.lower, CayleyStage::Containment)?,
            meter,
        )
    }

    fn multiply(&self, other: &Self, meter: &mut WorkMeter<'_>) -> Result<Self, CayleyError> {
        let products = [
            meter.multiply_rational(&self.lower, &other.lower, CayleyStage::Containment)?,
            meter.multiply_rational(&self.lower, &other.upper, CayleyStage::Containment)?,
            meter.multiply_rational(&self.upper, &other.lower, CayleyStage::Containment)?,
            meter.multiply_rational(&self.upper, &other.upper, CayleyStage::Containment)?,
        ];
        Self::hull(products, meter)
    }

    fn divide(&self, other: &Self, meter: &mut WorkMeter<'_>) -> Result<Option<Self>, CayleyError> {
        if other.contains_zero(meter)? {
            return Ok(None);
        }
        let quotients = [
            meter.divide_rational(&self.lower, &other.lower, CayleyStage::Containment)?,
            meter.divide_rational(&self.lower, &other.upper, CayleyStage::Containment)?,
            meter.divide_rational(&self.upper, &other.lower, CayleyStage::Containment)?,
            meter.divide_rational(&self.upper, &other.upper, CayleyStage::Containment)?,
        ];
        Self::hull(quotients, meter).map(Some)
    }

    fn hull(values: [BigRational; 4], meter: &mut WorkMeter<'_>) -> Result<Self, CayleyError> {
        let mut minimum = &values[0];
        let mut maximum = &values[0];
        for value in &values[1..] {
            if meter.compare_rational(value, minimum, CayleyStage::Containment)? == Ordering::Less {
                minimum = value;
            }
            if meter.compare_rational(value, maximum, CayleyStage::Containment)?
                == Ordering::Greater
            {
                maximum = value;
            }
        }
        Self::ordered(
            meter.clone_rational(minimum, CayleyStage::Containment)?,
            meter.clone_rational(maximum, CayleyStage::Containment)?,
            meter,
        )
    }

    fn ordered(
        lower: BigRational,
        upper: BigRational,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Self, CayleyError> {
        if meter.compare_rational(&lower, &upper, CayleyStage::Containment)? == Ordering::Greater {
            return Err(CayleyError::InvariantFailure {
                stage: CayleyStage::Containment,
            });
        }
        Ok(Self { lower, upper })
    }

    fn contains_zero(&self, meter: &mut WorkMeter<'_>) -> Result<bool, CayleyError> {
        meter.operation(CayleyStage::Containment)?;
        Ok(!self.lower.is_positive() && !self.upper.is_negative())
    }

    fn strict_sign(&self, meter: &mut WorkMeter<'_>) -> Result<Option<StrictSign>, CayleyError> {
        meter.operation(CayleyStage::Containment)?;
        Ok(if self.lower.is_positive() {
            Some(StrictSign::Positive)
        } else if self.upper.is_negative() {
            Some(StrictSign::Negative)
        } else {
            None
        })
    }

    fn strictly_inside_unit_interval(
        &self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<bool, CayleyError> {
        meter.operation(CayleyStage::Containment)?;
        Ok(self.lower.is_positive()
            && self.upper.is_positive()
            && self.upper.numer() < self.upper.denom())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StrictSign {
    Negative,
    Positive,
}

#[derive(Debug, Clone)]
struct ActualMillimetrePointInterval {
    coordinates: [ClosedRationalInterval; 3],
}

impl ActualMillimetrePointInterval {
    fn subtract(
        &self,
        other: &Self,
        meter: &mut WorkMeter<'_>,
    ) -> Result<[ClosedRationalInterval; 3], CayleyError> {
        try_array3(|axis| self.coordinates[axis].subtract(&other.coordinates[axis], meter))
    }

    fn interpolate(
        &self,
        end: &Self,
        parameter: &ClosedRationalInterval,
        meter: &mut WorkMeter<'_>,
    ) -> Result<Self, CayleyError> {
        let delta = end.subtract(self, meter)?;
        Ok(Self {
            coordinates: try_array3(|axis| {
                self.coordinates[axis].add(&delta[axis].multiply(parameter, meter)?, meter)
            })?,
        })
    }
}

#[derive(Debug, Clone)]
struct ActualMillimetreTriangleEnvelope {
    points: [ActualMillimetrePointInterval; 3],
}

fn classify_transversal_blocking_only(
    first: &ActualMillimetreTriangleEnvelope,
    second: &ActualMillimetreTriangleEnvelope,
    topology: PairTopology,
    limits: CayleyLimits,
) -> BlockingOnlyDecision {
    classify_transversal_metered(first, second, topology, limits)
        .map(|(decision, _)| decision)
        .unwrap_or(BlockingOnlyDecision::Unresolved)
}

fn classify_transversal_metered(
    first: &ActualMillimetreTriangleEnvelope,
    second: &ActualMillimetreTriangleEnvelope,
    topology: PairTopology,
    limits: CayleyLimits,
) -> Result<(BlockingOnlyDecision, CayleyWork), CayleyError> {
    let mut meter = WorkMeter::new(&limits);
    if !valid_triangle_input(first, &mut meter)? || !valid_triangle_input(second, &mut meter)? {
        return Ok((BlockingOnlyDecision::Unresolved, meter.work));
    }
    if matches!(topology, PairTopology::SharedHinge | PairTopology::SameFace) {
        return Ok((BlockingOnlyDecision::Unresolved, meter.work));
    }
    let shared = match topology {
        PairTopology::NoSharedFeature => None,
        PairTopology::SharedVertex {
            first_vertex,
            second_vertex,
        } if first_vertex < 3 && second_vertex < 3 => Some((first_vertex, second_vertex)),
        PairTopology::SharedVertex { .. } => {
            return Ok((BlockingOnlyDecision::Unresolved, meter.work));
        }
        PairTopology::SharedHinge | PairTopology::SameFace => unreachable!(),
    };

    if stable_projection(first, &mut meter)?.is_none()
        || stable_projection(second, &mut meter)?.is_none()
    {
        return Ok((BlockingOnlyDecision::Unresolved, meter.work));
    }

    let mut proven = false;
    for edge in 0..3 {
        if shared.is_none_or(|(vertex, _)| !edge_contains_vertex(edge, vertex))
            && edge_strictly_pierces_triangle(first, edge, second, &mut meter)?
        {
            proven = true;
        }
    }
    for edge in 0..3 {
        if shared.is_none_or(|(_, vertex)| !edge_contains_vertex(edge, vertex))
            && edge_strictly_pierces_triangle(second, edge, first, &mut meter)?
        {
            proven = true;
        }
    }
    Ok((
        if proven {
            BlockingOnlyDecision::ProvenPenetrating
        } else {
            BlockingOnlyDecision::Unresolved
        },
        meter.work,
    ))
}

fn valid_triangle_input(
    triangle: &ActualMillimetreTriangleEnvelope,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    for point in &triangle.points {
        for interval in &point.coordinates {
            if !valid_canonical_rational(&interval.lower, meter)?
                || !valid_canonical_rational(&interval.upper, meter)?
                || meter.compare_rational(
                    &interval.lower,
                    &interval.upper,
                    CayleyStage::Containment,
                )? == Ordering::Greater
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn valid_canonical_rational(
    value: &BigRational,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    meter.operation(CayleyStage::Containment)?;
    if !value.denom().is_positive() {
        return Ok(false);
    }
    Ok(meter
        .gcd_fallback(value.numer(), value.denom(), CayleyStage::Containment)?
        .is_one())
}

const fn edge_contains_vertex(edge: usize, vertex: usize) -> bool {
    edge == vertex || (edge + 1) % 3 == vertex
}

fn edge_strictly_pierces_triangle(
    source: &ActualMillimetreTriangleEnvelope,
    edge: usize,
    target: &ActualMillimetreTriangleEnvelope,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    let start = &source.points[edge];
    let end = &source.points[(edge + 1) % 3];
    let start_distance = signed_plane_distance(target, start, meter)?;
    let end_distance = signed_plane_distance(target, end, meter)?;
    let Some(start_sign) = start_distance.strict_sign(meter)? else {
        return Ok(false);
    };
    let Some(end_sign) = end_distance.strict_sign(meter)? else {
        return Ok(false);
    };
    if start_sign == end_sign {
        return Ok(false);
    }

    let denominator = start_distance.subtract(&end_distance, meter)?;
    if denominator.contains_zero(meter)? {
        return Ok(false);
    }
    let Some(parameter) = start_distance.divide(&denominator, meter)? else {
        return Ok(false);
    };
    if !parameter.strictly_inside_unit_interval(meter)? {
        return Ok(false);
    }
    let intersection = start.interpolate(end, &parameter, meter)?;
    point_strictly_inside_triangle_projection(&intersection, target, meter)
}

fn signed_plane_distance(
    triangle: &ActualMillimetreTriangleEnvelope,
    point: &ActualMillimetrePointInterval,
    meter: &mut WorkMeter<'_>,
) -> Result<ClosedRationalInterval, CayleyError> {
    let first = triangle.points[1].subtract(&triangle.points[0], meter)?;
    let second = triangle.points[2].subtract(&triangle.points[0], meter)?;
    let offset = point.subtract(&triangle.points[0], meter)?;
    determinant(&first, &second, &offset, meter)
}

fn stable_projection(
    triangle: &ActualMillimetreTriangleEnvelope,
    meter: &mut WorkMeter<'_>,
) -> Result<Option<[usize; 2]>, CayleyError> {
    let first = triangle.points[1].subtract(&triangle.points[0], meter)?;
    let second = triangle.points[2].subtract(&triangle.points[0], meter)?;
    let normal = cross(&first, &second, meter)?;
    for (axis, component) in normal.iter().enumerate() {
        if component.strict_sign(meter)?.is_some() {
            return Ok(Some(projected_axes(axis)));
        }
    }
    Ok(None)
}

fn point_strictly_inside_triangle_projection(
    point: &ActualMillimetrePointInterval,
    triangle: &ActualMillimetreTriangleEnvelope,
    meter: &mut WorkMeter<'_>,
) -> Result<bool, CayleyError> {
    let Some([first_axis, second_axis]) = stable_projection(triangle, meter)? else {
        return Ok(false);
    };
    for edge in 0..3 {
        let start = &triangle.points[edge];
        let end = &triangle.points[(edge + 1) % 3];
        let opposite = &triangle.points[(edge + 2) % 3];
        let point_side = projected_orientation(start, end, point, first_axis, second_axis, meter)?;
        let interior_side =
            projected_orientation(start, end, opposite, first_axis, second_axis, meter)?;
        let Some(point_sign) = point_side.strict_sign(meter)? else {
            return Ok(false);
        };
        let Some(interior_sign) = interior_side.strict_sign(meter)? else {
            return Ok(false);
        };
        if point_sign != interior_sign {
            return Ok(false);
        }
    }
    Ok(true)
}

fn projected_orientation(
    start: &ActualMillimetrePointInterval,
    end: &ActualMillimetrePointInterval,
    point: &ActualMillimetrePointInterval,
    first_axis: usize,
    second_axis: usize,
    meter: &mut WorkMeter<'_>,
) -> Result<ClosedRationalInterval, CayleyError> {
    let line_first = end.coordinates[first_axis].subtract(&start.coordinates[first_axis], meter)?;
    let line_second =
        end.coordinates[second_axis].subtract(&start.coordinates[second_axis], meter)?;
    let point_first =
        point.coordinates[first_axis].subtract(&start.coordinates[first_axis], meter)?;
    let point_second =
        point.coordinates[second_axis].subtract(&start.coordinates[second_axis], meter)?;
    let first_product = line_first.multiply(&point_second, meter)?;
    let second_product = line_second.multiply(&point_first, meter)?;
    first_product.subtract(&second_product, meter)
}

fn determinant(
    first: &[ClosedRationalInterval; 3],
    second: &[ClosedRationalInterval; 3],
    third: &[ClosedRationalInterval; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<ClosedRationalInterval, CayleyError> {
    let crossed = cross(second, third, meter)?;
    let first_term = first[0].multiply(&crossed[0], meter)?;
    let second_term = first[1].multiply(&crossed[1], meter)?;
    let third_term = first[2].multiply(&crossed[2], meter)?;
    first_term.add(&second_term, meter)?.add(&third_term, meter)
}

fn cross(
    first: &[ClosedRationalInterval; 3],
    second: &[ClosedRationalInterval; 3],
    meter: &mut WorkMeter<'_>,
) -> Result<[ClosedRationalInterval; 3], CayleyError> {
    try_array3(|axis| {
        let next = (axis + 1) % 3;
        let last = (axis + 2) % 3;
        let forward = first[next].multiply(&second[last], meter)?;
        let backward = first[last].multiply(&second[next], meter)?;
        forward.subtract(&backward, meter)
    })
}

const fn projected_axes(drop_axis: usize) -> [usize; 2] {
    match drop_axis {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    }
}

fn try_array3<T>(
    mut element: impl FnMut(usize) -> Result<T, CayleyError>,
) -> Result<[T; 3], CayleyError> {
    Ok([element(0)?, element(1)?, element(2)?])
}

#[cfg(test)]
mod tests {
    use num_bigint::BigInt;
    use num_traits::{One, Zero};

    use super::*;

    fn rational(numerator: i64, denominator: i64) -> BigRational {
        BigRational::new(BigInt::from(numerator), BigInt::from(denominator))
    }

    fn point(x: i64, y: i64, z: i64) -> ActualMillimetrePointInterval {
        ActualMillimetrePointInterval {
            coordinates: [x, y, z].map(|value| {
                ClosedRationalInterval::point(BigRational::from_integer(value.into()))
            }),
        }
    }

    fn inflated_point(coordinates: [i64; 3], radius: BigRational) -> ActualMillimetrePointInterval {
        ActualMillimetrePointInterval {
            coordinates: coordinates.map(|value| {
                ClosedRationalInterval::inflated(
                    BigRational::from_integer(value.into()),
                    radius.clone(),
                )
            }),
        }
    }

    fn horizontal_target() -> ActualMillimetreTriangleEnvelope {
        ActualMillimetreTriangleEnvelope {
            points: [point(-2, -2, 0), point(2, -2, 0), point(0, 2, 0)],
        }
    }

    fn robust_piercer() -> ActualMillimetreTriangleEnvelope {
        ActualMillimetreTriangleEnvelope {
            points: [point(0, 0, -1), point(0, 0, 1), point(1, 0, 1)],
        }
    }

    fn inflated_triangle(
        triangle: &ActualMillimetreTriangleEnvelope,
        radius: BigRational,
    ) -> ActualMillimetreTriangleEnvelope {
        ActualMillimetreTriangleEnvelope {
            points: triangle
                .points
                .clone()
                .map(|point| ActualMillimetrePointInterval {
                    coordinates: point.coordinates.map(|coordinate| {
                        ClosedRationalInterval::inflated(coordinate.lower, radius.clone())
                    }),
                }),
        }
    }

    const TRIANGLE_PERMUTATIONS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];

    fn permuted_triangle(
        triangle: &ActualMillimetreTriangleEnvelope,
        permutation: [usize; 3],
    ) -> ActualMillimetreTriangleEnvelope {
        ActualMillimetreTriangleEnvelope {
            points: permutation.map(|index| triangle.points[index].clone()),
        }
    }

    fn transformed_triangle(
        triangle: &ActualMillimetreTriangleEnvelope,
        scale: BigRational,
        translation: [BigRational; 3],
    ) -> ActualMillimetreTriangleEnvelope {
        assert!(scale.is_positive());
        ActualMillimetreTriangleEnvelope {
            points: triangle
                .points
                .clone()
                .map(|point| ActualMillimetrePointInterval {
                    coordinates: std::array::from_fn(|axis| ClosedRationalInterval {
                        lower: &point.coordinates[axis].lower * &scale + &translation[axis],
                        upper: &point.coordinates[axis].upper * &scale + &translation[axis],
                    }),
                }),
        }
    }

    fn coordinate_permuted_and_reflected_triangle(
        triangle: &ActualMillimetreTriangleEnvelope,
        permutation: [usize; 3],
        reflection_mask: u8,
    ) -> ActualMillimetreTriangleEnvelope {
        ActualMillimetreTriangleEnvelope {
            points: triangle
                .points
                .clone()
                .map(|point| ActualMillimetrePointInterval {
                    coordinates: std::array::from_fn(|axis| {
                        let source = &point.coordinates[permutation[axis]];
                        if reflection_mask & (1 << axis) == 0 {
                            source.clone()
                        } else {
                            ClosedRationalInterval {
                                lower: -source.upper.clone(),
                                upper: -source.lower.clone(),
                            }
                        }
                    }),
                }),
        }
    }

    #[test]
    fn robust_actual_mm_transversal_is_the_only_positive_result() {
        let radius = rational(1, 10_000);
        let first = inflated_triangle(&robust_piercer(), radius.clone());
        let second = inflated_triangle(&horizontal_target(), radius);
        assert_eq!(
            classify_transversal_blocking_only(
                &first,
                &second,
                PairTopology::NoSharedFeature,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::ProvenPenetrating
        );
        assert_eq!(
            classify_transversal_blocking_only(
                &second,
                &first,
                PairTopology::NoSharedFeature,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::ProvenPenetrating
        );
    }

    #[test]
    fn point_line_and_coplanar_contact_never_become_penetration() {
        let target = horizontal_target();
        let point_touch = ActualMillimetreTriangleEnvelope {
            points: [point(0, 0, 0), point(0, 0, 1), point(1, 0, 1)],
        };
        let line_touch = ActualMillimetreTriangleEnvelope {
            points: [point(-1, -2, 0), point(1, -2, 0), point(0, -2, 1)],
        };
        let coplanar = ActualMillimetreTriangleEnvelope {
            points: [point(-1, -1, 0), point(1, -1, 0), point(0, 1, 0)],
        };
        for candidate in [&point_touch, &line_touch, &coplanar] {
            assert_eq!(
                classify_transversal_blocking_only(
                    candidate,
                    &target,
                    PairTopology::NoSharedFeature,
                    CayleyLimits::default(),
                ),
                BlockingOnlyDecision::Unresolved
            );
        }
    }

    #[test]
    fn shared_vertex_excludes_only_incident_edges_and_never_shared_point_contact() {
        let target = horizontal_target();
        let strict_away_from_shared = ActualMillimetreTriangleEnvelope {
            points: [point(-2, -2, 0), point(0, 0, -1), point(0, 0, 1)],
        };
        assert_eq!(
            classify_transversal_blocking_only(
                &strict_away_from_shared,
                &target,
                PairTopology::SharedVertex {
                    first_vertex: 0,
                    second_vertex: 0,
                },
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::ProvenPenetrating
        );

        let shared_point_only = ActualMillimetreTriangleEnvelope {
            points: [point(-2, -2, 0), point(-2, -2, 1), point(-1, -2, 1)],
        };
        assert_eq!(
            classify_transversal_blocking_only(
                &shared_point_only,
                &target,
                PairTopology::SharedVertex {
                    first_vertex: 0,
                    second_vertex: 0,
                },
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::Unresolved
        );

        for first_permutation in TRIANGLE_PERMUTATIONS {
            for second_permutation in TRIANGLE_PERMUTATIONS {
                let first = permuted_triangle(&strict_away_from_shared, first_permutation);
                let second = permuted_triangle(&target, second_permutation);
                let first_vertex = first_permutation
                    .iter()
                    .position(|source| *source == 0)
                    .expect("shared first vertex");
                let second_vertex = second_permutation
                    .iter()
                    .position(|source| *source == 0)
                    .expect("shared second vertex");
                assert_eq!(
                    classify_transversal_blocking_only(
                        &first,
                        &second,
                        PairTopology::SharedVertex {
                            first_vertex,
                            second_vertex,
                        },
                        CayleyLimits::default(),
                    ),
                    BlockingOnlyDecision::ProvenPenetrating
                );
            }
        }
    }

    #[test]
    fn shared_hinge_same_face_and_invalid_shared_vertex_are_sealed_unresolved() {
        let first = robust_piercer();
        let second = horizontal_target();
        for topology in [
            PairTopology::SharedHinge,
            PairTopology::SameFace,
            PairTopology::SharedVertex {
                first_vertex: 3,
                second_vertex: 0,
            },
        ] {
            assert_eq!(
                classify_transversal_blocking_only(
                    &first,
                    &second,
                    topology,
                    CayleyLimits::default(),
                ),
                BlockingOnlyDecision::Unresolved
            );
        }
    }

    #[test]
    fn denominator_parameter_and_interior_margins_must_all_be_strict() {
        let target = horizontal_target();
        let uncertain_endpoint = ActualMillimetreTriangleEnvelope {
            points: [
                inflated_point([0, 0, 0], BigRational::one()),
                point(0, 0, 1),
                point(1, 0, 1),
            ],
        };
        let boundary_piercer = ActualMillimetreTriangleEnvelope {
            points: [point(0, -2, -1), point(0, -2, 1), point(1, -2, 1)],
        };
        let zero_width = ActualMillimetreTriangleEnvelope {
            points: [
                point(0, 0, -1),
                ActualMillimetrePointInterval {
                    coordinates: [
                        ClosedRationalInterval::point(BigRational::zero()),
                        ClosedRationalInterval::point(BigRational::zero()),
                        ClosedRationalInterval {
                            lower: -BigRational::one(),
                            upper: BigRational::one(),
                        },
                    ],
                },
                point(1, 0, 1),
            ],
        };
        for candidate in [&uncertain_endpoint, &boundary_piercer, &zero_width] {
            assert_eq!(
                classify_transversal_blocking_only(
                    candidate,
                    &target,
                    PairTopology::NoSharedFeature,
                    CayleyLimits::default(),
                ),
                BlockingOnlyDecision::Unresolved
            );
        }
    }

    #[test]
    fn every_vertex_permutation_and_pair_direction_preserves_the_result() {
        let first = robust_piercer();
        let second = horizontal_target();
        for first_permutation in TRIANGLE_PERMUTATIONS {
            for second_permutation in TRIANGLE_PERMUTATIONS {
                let reordered_first = permuted_triangle(&first, first_permutation);
                let reordered_second = permuted_triangle(&second, second_permutation);
                for (left, right) in [
                    (&reordered_first, &reordered_second),
                    (&reordered_second, &reordered_first),
                ] {
                    assert_eq!(
                        classify_transversal_blocking_only(
                            left,
                            right,
                            PairTopology::NoSharedFeature,
                            CayleyLimits::default(),
                        ),
                        BlockingOnlyDecision::ProvenPenetrating
                    );
                }
            }
        }
    }

    #[test]
    fn every_coordinate_axis_and_reflection_preserves_the_robust_result() {
        let first = robust_piercer();
        let second = horizontal_target();
        let mut observed_normal_axes = [false; 3];
        for coordinate_permutation in TRIANGLE_PERMUTATIONS {
            let normal_axis = coordinate_permutation
                .iter()
                .position(|source_axis| *source_axis == 2)
                .expect("original Z normal must remain on one coordinate axis");
            observed_normal_axes[normal_axis] = true;
            for reflection_mask in 0..8 {
                let transformed_first = coordinate_permuted_and_reflected_triangle(
                    &first,
                    coordinate_permutation,
                    reflection_mask,
                );
                let transformed_second = coordinate_permuted_and_reflected_triangle(
                    &second,
                    coordinate_permutation,
                    reflection_mask,
                );
                for (left, right) in [
                    (&transformed_first, &transformed_second),
                    (&transformed_second, &transformed_first),
                ] {
                    assert_eq!(
                        classify_transversal_blocking_only(
                            left,
                            right,
                            PairTopology::NoSharedFeature,
                            CayleyLimits::default(),
                        ),
                        BlockingOnlyDecision::ProvenPenetrating,
                        "coordinate permutation {coordinate_permutation:?}, reflection mask \
                         {reflection_mask:#05b}",
                    );
                }
            }
        }
        assert_eq!(observed_normal_axes, [true; 3]);
    }

    #[test]
    fn positive_scale_translation_and_strict_radius_margin_are_invariant() {
        let first = robust_piercer();
        let second = horizontal_target();
        for (scale, translation) in [
            (
                rational(1, 100),
                [
                    BigRational::from_integer((-1_000_000_000_000_i64).into()),
                    rational(7, 3),
                    BigRational::from_integer(1_000_000_000_000_i64.into()),
                ],
            ),
            (
                BigRational::one(),
                [
                    BigRational::zero(),
                    BigRational::zero(),
                    BigRational::zero(),
                ],
            ),
            (
                BigRational::from_integer(1_000_000_i64.into()),
                [
                    BigRational::from_integer(1_000_000_000_000_i64.into()),
                    rational(-11, 5),
                    BigRational::from_integer((-1_000_000_000_000_i64).into()),
                ],
            ),
        ] {
            let transformed_first =
                transformed_triangle(&first, scale.clone(), translation.clone());
            let transformed_second = transformed_triangle(&second, scale, translation);
            assert_eq!(
                classify_transversal_blocking_only(
                    &transformed_first,
                    &transformed_second,
                    PairTopology::NoSharedFeature,
                    CayleyLimits::default(),
                ),
                BlockingOnlyDecision::ProvenPenetrating
            );
        }

        for (radius, expected) in [
            (rational(1, 10_000), BlockingOnlyDecision::ProvenPenetrating),
            (BigRational::one(), BlockingOnlyDecision::Unresolved),
        ] {
            assert_eq!(
                classify_transversal_blocking_only(
                    &inflated_triangle(&first, radius.clone()),
                    &inflated_triangle(&second, radius),
                    PairTopology::NoSharedFeature,
                    CayleyLimits::default(),
                ),
                expected
            );
        }
    }

    #[test]
    fn strict_radius_threshold_has_a_certified_rational_bracket() {
        let first = robust_piercer();
        let second = horizontal_target();
        let denominator = 1_000_000_000_000_000_i64;
        let below = rational(66_521_651_620_659, denominator);
        let above = rational(66_521_651_620_660, denominator);

        // At this fixture's first edge, the strict t.upper < 1 margin changes
        // sign with 16r^3 + 46r^2 + 57r - 4. Its unique positive root lies
        // inside this one-quadrillionth-wide rational bracket.
        let threshold_polynomial = |radius: &BigRational| {
            BigRational::from_integer(16.into()) * radius.pow(3)
                + BigRational::from_integer(46.into()) * radius.pow(2)
                + BigRational::from_integer(57.into()) * radius
                - BigRational::from_integer(4.into())
        };
        assert!(threshold_polynomial(&below).is_negative());
        assert!(threshold_polynomial(&above).is_positive());

        // The rational-root theorem leaves only these positive candidates.
        // None is a root, so an exact equality fixture cannot be represented
        // by the BigRational radius accepted by this primitive.
        for candidate in [
            rational(1, 16),
            rational(1, 8),
            rational(1, 4),
            rational(1, 2),
            BigRational::one(),
            rational(2, 1),
            rational(4, 1),
        ] {
            assert!(!threshold_polynomial(&candidate).is_zero());
        }

        assert_eq!(
            classify_transversal_blocking_only(
                &inflated_triangle(&first, below.clone()),
                &inflated_triangle(&second, below),
                PairTopology::NoSharedFeature,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::ProvenPenetrating,
        );
        assert_eq!(
            classify_transversal_blocking_only(
                &inflated_triangle(&first, above.clone()),
                &inflated_triangle(&second, above),
                PairTopology::NoSharedFeature,
                CayleyLimits::default(),
            ),
            BlockingOnlyDecision::Unresolved,
        );
    }

    #[test]
    fn malformed_interval_or_noncanonical_ratio_is_unresolved() {
        let second = horizontal_target();

        let mut reversed = robust_piercer();
        reversed.points[0].coordinates[0] = ClosedRationalInterval {
            lower: BigRational::one(),
            upper: BigRational::zero(),
        };

        let mut noncanonical = robust_piercer();
        noncanonical.points[0].coordinates[0].lower = BigRational::new_raw(2.into(), 2.into());

        let mut negative_denominator = robust_piercer();
        negative_denominator.points[0].coordinates[0].lower =
            BigRational::new_raw(1.into(), (-1).into());

        for malformed in [&reversed, &noncanonical, &negative_denominator] {
            assert_eq!(
                classify_transversal_blocking_only(
                    malformed,
                    &second,
                    PairTopology::NoSharedFeature,
                    CayleyLimits::default(),
                ),
                BlockingOnlyDecision::Unresolved
            );
        }
    }

    #[test]
    fn degenerate_triangles_and_inflated_shared_point_contact_are_unresolved() {
        let robust = robust_piercer();
        let collinear = ActualMillimetreTriangleEnvelope {
            points: [point(-2, 0, 0), point(0, 0, 0), point(2, 0, 0)],
        };
        let collapsed = ActualMillimetreTriangleEnvelope {
            points: [point(0, 0, 0), point(0, 0, 0), point(0, 0, 0)],
        };
        for degenerate in [&collinear, &collapsed] {
            for (first, second) in [(&robust, degenerate), (degenerate, &robust)] {
                assert_eq!(
                    classify_transversal_blocking_only(
                        first,
                        second,
                        PairTopology::NoSharedFeature,
                        CayleyLimits::default(),
                    ),
                    BlockingOnlyDecision::Unresolved,
                );
            }
        }

        let radius = rational(1, 10_000);
        let target = inflated_triangle(&horizontal_target(), radius.clone());
        let shared_point_only = inflated_triangle(
            &ActualMillimetreTriangleEnvelope {
                points: [point(-2, -2, 0), point(-2, -2, 1), point(-1, -2, 1)],
            },
            radius,
        );
        for (first, second) in [(&shared_point_only, &target), (&target, &shared_point_only)] {
            assert_eq!(
                classify_transversal_blocking_only(
                    first,
                    second,
                    PairTopology::SharedVertex {
                        first_vertex: 0,
                        second_vertex: 0,
                    },
                    CayleyLimits::default(),
                ),
                BlockingOnlyDecision::Unresolved,
            );
        }
    }

    #[test]
    fn a_later_resource_failure_revokes_an_already_observed_witness() {
        let first = robust_piercer();
        let second = horizontal_target();
        let default_limits = CayleyLimits::default();
        let mut prefix_meter = WorkMeter::new(&default_limits);
        assert!(valid_triangle_input(&first, &mut prefix_meter).expect("first input"));
        assert!(valid_triangle_input(&second, &mut prefix_meter).expect("second input"));
        assert!(
            stable_projection(&first, &mut prefix_meter)
                .expect("first projection")
                .is_some()
        );
        assert!(
            stable_projection(&second, &mut prefix_meter)
                .expect("second projection")
                .is_some()
        );
        assert!(
            edge_strictly_pierces_triangle(&first, 0, &second, &mut prefix_meter)
                .expect("first edge witness")
        );
        let operations_after_witness = prefix_meter.work.interval_operations;

        let mut one_short_for_the_next_operation = default_limits;
        one_short_for_the_next_operation.max_interval_operations = operations_after_witness;
        let mut exact_prefix_meter = WorkMeter::new(&one_short_for_the_next_operation);
        assert!(valid_triangle_input(&first, &mut exact_prefix_meter).expect("first exact input"));
        assert!(
            valid_triangle_input(&second, &mut exact_prefix_meter).expect("second exact input")
        );
        assert!(
            stable_projection(&first, &mut exact_prefix_meter)
                .expect("first exact projection")
                .is_some()
        );
        assert!(
            stable_projection(&second, &mut exact_prefix_meter)
                .expect("second exact projection")
                .is_some()
        );
        assert!(
            edge_strictly_pierces_triangle(&first, 0, &second, &mut exact_prefix_meter)
                .expect("witness at the exact prefix limit")
        );
        assert_eq!(
            exact_prefix_meter.work.interval_operations,
            operations_after_witness
        );
        assert!(matches!(
            edge_strictly_pierces_triangle(&first, 1, &second, &mut exact_prefix_meter),
            Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Containment,
                resource: "interval_operations",
            })
        ));
        assert_eq!(
            classify_transversal_blocking_only(
                &first,
                &second,
                PairTopology::NoSharedFeature,
                one_short_for_the_next_operation,
            ),
            BlockingOnlyDecision::Unresolved,
        );
    }

    #[test]
    fn exact_resource_limit_accepts_observed_work_and_one_short_fails_closed() {
        let first = robust_piercer();
        let second = horizontal_target();
        let (_, work) = classify_transversal_metered(
            &first,
            &second,
            PairTopology::NoSharedFeature,
            CayleyLimits::default(),
        )
        .expect("baseline robust predicate");
        assert!(work.interval_operations > 0);

        let exact = CayleyLimits {
            max_interval_operations: work.interval_operations,
            max_intermediate_bits: work.max_preflight_bits.max(work.max_observed_bits),
            max_gcd_fallback_calls: work.gcd_fallback_calls,
            max_gcd_fallback_input_bits: work.gcd_fallback_input_bits,
            max_rational_allocations: work.rational_allocations,
            max_rational_allocation_bits: work.max_rational_allocation_bits,
            max_total_rational_allocation_bits: work.total_rational_allocation_bits,
            ..CayleyLimits::default()
        };
        assert_eq!(
            classify_transversal_metered(&first, &second, PairTopology::NoSharedFeature, exact,)
                .expect("exact observed resource limits")
                .0,
            BlockingOnlyDecision::ProvenPenetrating
        );

        macro_rules! one_short {
            ($field:ident) => {{
                let mut one_short = exact;
                assert!(
                    one_short.$field > 0,
                    "{} must be exercised",
                    stringify!($field)
                );
                one_short.$field -= 1;
                assert_eq!(
                    classify_transversal_blocking_only(
                        &first,
                        &second,
                        PairTopology::NoSharedFeature,
                        one_short,
                    ),
                    BlockingOnlyDecision::Unresolved,
                    "{} one-short must fail closed",
                    stringify!($field)
                );
            }};
        }

        one_short!(max_interval_operations);
        one_short!(max_intermediate_bits);
        one_short!(max_gcd_fallback_calls);
        one_short!(max_gcd_fallback_input_bits);
        one_short!(max_rational_allocations);
        one_short!(max_rational_allocation_bits);
        one_short!(max_total_rational_allocation_bits);
    }
}
