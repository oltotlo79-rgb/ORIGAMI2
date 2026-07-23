use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};
use ori_domain::Point2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactConvexOverlapLimitsV1 {
    pub max_input_vertices: usize,
    pub max_output_vertices: usize,
    pub max_operations: usize,
    pub max_integer_bits: u64,
    pub max_bit_work: u64,
}

impl Default for ExactConvexOverlapLimitsV1 {
    fn default() -> Self {
        Self {
            max_input_vertices: 64,
            max_output_vertices: 256,
            max_operations: 16_384,
            max_integer_bits: 16_384,
            max_bit_work: 4_000_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactConvexOverlapErrorV1 {
    InvalidInput,
    ResourceLimit,
    Unrepresentable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactPoint2V1 {
    x: BigRational,
    y: BigRational,
}

impl ExactPoint2V1 {
    pub fn x(&self) -> &BigRational {
        &self.x
    }
    pub fn y(&self) -> &BigRational {
        &self.y
    }
    pub fn rounded(&self) -> Option<Point2> {
        Some(Point2::new(
            self.x.to_f64()?.then_finite()?,
            self.y.to_f64()?.then_finite()?,
        ))
    }
}

trait FiniteF64 {
    fn then_finite(self) -> Option<f64>;
}
impl FiniteF64 for f64 {
    fn then_finite(self) -> Option<f64> {
        self.is_finite().then_some(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactConvexOverlapV1 {
    points: Vec<ExactPoint2V1>,
    signed_double_area: BigRational,
    operations: usize,
    bit_work: u64,
    maximum_integer_bits: u64,
}

impl ExactConvexOverlapV1 {
    pub fn points(&self) -> &[ExactPoint2V1] {
        &self.points
    }
    pub fn signed_double_area(&self) -> &BigRational {
        &self.signed_double_area
    }
    pub fn has_positive_area(&self) -> bool {
        self.signed_double_area.is_positive()
    }
    pub fn operations(&self) -> usize {
        self.operations
    }
    pub fn bit_work(&self) -> u64 {
        self.bit_work
    }
    pub fn maximum_integer_bits(&self) -> u64 {
        self.maximum_integer_bits
    }
}

struct Meter {
    operations: usize,
    bit_work: u64,
    maximum_integer_bits: u64,
    limits: ExactConvexOverlapLimitsV1,
}
impl Meter {
    fn charge(
        &mut self,
        operands: &[&BigRational],
        predicted_bits: u64,
    ) -> Result<(), ExactConvexOverlapErrorV1> {
        self.operations = self
            .operations
            .checked_add(1)
            .ok_or(ExactConvexOverlapErrorV1::ResourceLimit)?;
        (self.operations <= self.limits.max_operations)
            .then_some(())
            .ok_or(ExactConvexOverlapErrorV1::ResourceLimit)?;
        let operand_work = operands.iter().try_fold(0u64, |sum, value| {
            self.check(value)?;
            sum.checked_add(value.numer().bits().saturating_add(value.denom().bits()))
                .ok_or(ExactConvexOverlapErrorV1::ResourceLimit)
        })?;
        if predicted_bits > self.limits.max_integer_bits {
            return Err(ExactConvexOverlapErrorV1::ResourceLimit);
        }
        self.bit_work = self
            .bit_work
            .checked_add(operand_work.saturating_add(predicted_bits))
            .ok_or(ExactConvexOverlapErrorV1::ResourceLimit)?;
        (self.bit_work <= self.limits.max_bit_work)
            .then_some(())
            .ok_or(ExactConvexOverlapErrorV1::ResourceLimit)
    }
    fn check(&mut self, value: &BigRational) -> Result<(), ExactConvexOverlapErrorV1> {
        let bits = value.numer().bits().max(value.denom().bits());
        self.maximum_integer_bits = self.maximum_integer_bits.max(bits);
        if bits > self.limits.max_integer_bits {
            return Err(ExactConvexOverlapErrorV1::ResourceLimit);
        }
        Ok(())
    }
    fn input(&mut self, value: BigRational) -> Result<BigRational, ExactConvexOverlapErrorV1> {
        self.check(&value)?;
        self.charge(&[&value], value.numer().bits().max(value.denom().bits()))?;
        Ok(value)
    }
    fn add(
        &mut self,
        a: &BigRational,
        b: &BigRational,
    ) -> Result<BigRational, ExactConvexOverlapErrorV1> {
        let predicted = (a.numer().bits() + b.denom().bits())
            .max(b.numer().bits() + a.denom().bits())
            .saturating_add(1)
            .max(a.denom().bits() + b.denom().bits());
        self.charge(&[a, b], predicted)?;
        let value = a + b;
        self.check(&value)?;
        Ok(value)
    }
    fn sub(
        &mut self,
        a: &BigRational,
        b: &BigRational,
    ) -> Result<BigRational, ExactConvexOverlapErrorV1> {
        let predicted = (a.numer().bits() + b.denom().bits())
            .max(b.numer().bits() + a.denom().bits())
            .saturating_add(1)
            .max(a.denom().bits() + b.denom().bits());
        self.charge(&[a, b], predicted)?;
        let value = a - b;
        self.check(&value)?;
        Ok(value)
    }
    fn mul(
        &mut self,
        a: &BigRational,
        b: &BigRational,
    ) -> Result<BigRational, ExactConvexOverlapErrorV1> {
        let predicted =
            (a.numer().bits() + b.numer().bits()).max(a.denom().bits() + b.denom().bits());
        self.charge(&[a, b], predicted)?;
        let value = a * b;
        self.check(&value)?;
        Ok(value)
    }
    fn div(
        &mut self,
        a: &BigRational,
        b: &BigRational,
    ) -> Result<BigRational, ExactConvexOverlapErrorV1> {
        if b.is_zero() {
            return Err(ExactConvexOverlapErrorV1::Unrepresentable);
        }
        let predicted =
            (a.numer().bits() + b.denom().bits()).max(a.denom().bits() + b.numer().bits());
        self.charge(&[a, b], predicted)?;
        let value = a / b;
        self.check(&value)?;
        Ok(value)
    }
}

fn point(point: Point2, meter: &mut Meter) -> Result<ExactPoint2V1, ExactConvexOverlapErrorV1> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return Err(ExactConvexOverlapErrorV1::InvalidInput);
    }
    Ok(ExactPoint2V1 {
        x: meter.input(
            BigRational::from_float(if point.x == 0.0 { 0.0 } else { point.x })
                .ok_or(ExactConvexOverlapErrorV1::InvalidInput)?,
        )?,
        y: meter.input(
            BigRational::from_float(if point.y == 0.0 { 0.0 } else { point.y })
                .ok_or(ExactConvexOverlapErrorV1::InvalidInput)?,
        )?,
    })
}

fn cross(
    a: &ExactPoint2V1,
    b: &ExactPoint2V1,
    p: &ExactPoint2V1,
    meter: &mut Meter,
) -> Result<BigRational, ExactConvexOverlapErrorV1> {
    let bax = meter.sub(&b.x, &a.x)?;
    let pay = meter.sub(&p.y, &a.y)?;
    let bay = meter.sub(&b.y, &a.y)?;
    let pax = meter.sub(&p.x, &a.x)?;
    let left = meter.mul(&bax, &pay)?;
    let right = meter.mul(&bay, &pax)?;
    meter.sub(&left, &right)
}

fn area(
    points: &[ExactPoint2V1],
    meter: &mut Meter,
) -> Result<BigRational, ExactConvexOverlapErrorV1> {
    let mut value = BigRational::zero();
    for index in 0..points.len() {
        let a = &points[index];
        let b = &points[(index + 1) % points.len()];
        let left = meter.mul(&a.x, &b.y)?;
        let right = meter.mul(&a.y, &b.x)?;
        let term = meter.sub(&left, &right)?;
        value = meter.add(&value, &term)?;
    }
    Ok(value)
}

fn prepare(
    points: &[Point2],
    meter: &mut Meter,
) -> Result<Vec<ExactPoint2V1>, ExactConvexOverlapErrorV1> {
    if points.len() < 3 || points.len() > meter.limits.max_input_vertices {
        return Err(ExactConvexOverlapErrorV1::InvalidInput);
    }
    let mut exact = points
        .iter()
        .copied()
        .map(|p| point(p, meter))
        .collect::<Result<Vec<_>, _>>()?;
    if exact
        .iter()
        .enumerate()
        .any(|(i, p)| p == &exact[(i + 1) % exact.len()])
    {
        return Err(ExactConvexOverlapErrorV1::InvalidInput);
    }
    let signed = area(&exact, meter)?;
    if signed.is_zero() {
        return Err(ExactConvexOverlapErrorV1::InvalidInput);
    }
    if signed.is_negative() {
        exact.reverse();
    }
    for index in 0..exact.len() {
        if cross(
            &exact[index],
            &exact[(index + 1) % exact.len()],
            &exact[(index + 2) % exact.len()],
            meter,
        )?
        .is_negative()
        {
            return Err(ExactConvexOverlapErrorV1::InvalidInput);
        }
    }
    Ok(exact)
}

fn line_intersection(
    previous: &ExactPoint2V1,
    current: &ExactPoint2V1,
    start: &ExactPoint2V1,
    end: &ExactPoint2V1,
    meter: &mut Meter,
) -> Result<ExactPoint2V1, ExactConvexOverlapErrorV1> {
    let rx = meter.sub(&current.x, &previous.x)?;
    let ry = meter.sub(&current.y, &previous.y)?;
    let sx = meter.sub(&end.x, &start.x)?;
    let sy = meter.sub(&end.y, &start.y)?;
    let rxs = meter.mul(&rx, &sy)?;
    let rys = meter.mul(&ry, &sx)?;
    let denominator = meter.sub(&rxs, &rys)?;
    if denominator.is_zero() {
        return Err(ExactConvexOverlapErrorV1::Unrepresentable);
    }
    let qpx = meter.sub(&start.x, &previous.x)?;
    let qpy = meter.sub(&start.y, &previous.y)?;
    let a = meter.mul(&qpx, &sy)?;
    let b = meter.mul(&qpy, &sx)?;
    let numerator = meter.sub(&a, &b)?;
    let t = meter.div(&numerator, &denominator)?;
    let xt = meter.mul(&rx, &t)?;
    let yt = meter.mul(&ry, &t)?;
    Ok(ExactPoint2V1 {
        x: meter.add(&previous.x, &xt)?,
        y: meter.add(&previous.y, &yt)?,
    })
}

pub fn exact_convex_polygon_overlap_v1(
    subject: &[Point2],
    clip: &[Point2],
    limits: ExactConvexOverlapLimitsV1,
) -> Result<ExactConvexOverlapV1, ExactConvexOverlapErrorV1> {
    if limits.max_input_vertices < 3
        || limits.max_output_vertices < 3
        || limits.max_operations == 0
        || limits.max_integer_bits == 0
        || limits.max_bit_work == 0
    {
        return Err(ExactConvexOverlapErrorV1::ResourceLimit);
    }
    let mut meter = Meter {
        operations: 0,
        bit_work: 0,
        maximum_integer_bits: 0,
        limits,
    };
    let mut output = prepare(subject, &mut meter)?;
    let clip = prepare(clip, &mut meter)?;
    for index in 0..clip.len() {
        let start = &clip[index];
        let end = &clip[(index + 1) % clip.len()];
        let input = std::mem::take(&mut output);
        if input.is_empty() {
            break;
        }
        output = Vec::with_capacity(input.len().min(limits.max_output_vertices));
        for edge in 0..input.len() {
            let previous = &input[(edge + input.len() - 1) % input.len()];
            let current = &input[edge];
            let previous_inside = !cross(start, end, previous, &mut meter)?.is_negative();
            let current_inside = !cross(start, end, current, &mut meter)?.is_negative();
            if previous_inside != current_inside {
                if output.len() >= limits.max_output_vertices {
                    return Err(ExactConvexOverlapErrorV1::ResourceLimit);
                }
                output.push(line_intersection(
                    previous, current, start, end, &mut meter,
                )?);
            }
            if current_inside {
                if output.len() >= limits.max_output_vertices {
                    return Err(ExactConvexOverlapErrorV1::ResourceLimit);
                }
                output.push(current.clone());
            }
        }
    }
    let signed_double_area = if output.len() >= 3 {
        area(&output, &mut meter)?
    } else {
        BigRational::zero()
    };
    if signed_double_area.is_negative() {
        output.reverse();
    }
    if let Some((start, _)) = output
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.x.cmp(&b.x).then(a.y.cmp(&b.y)))
    {
        output.rotate_left(start);
    }
    Ok(ExactConvexOverlapV1 {
        points: output,
        signed_double_area: signed_double_area.abs(),
        operations: meter.operations,
        bit_work: meter.bit_work,
        maximum_integer_bits: meter.maximum_integer_bits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rectangle_with_vertices(count: usize) -> Vec<Point2> {
        assert!(count >= 4 && count.is_multiple_of(4));
        let per_side = count / 4;
        let mut points = Vec::new();
        for side in 0..4 {
            for step in 0..per_side {
                let t = step as f64 / per_side as f64;
                points.push(match side {
                    0 => Point2::new(t * 8.0, 0.0),
                    1 => Point2::new(8.0, t * 8.0),
                    2 => Point2::new(8.0 - t * 8.0, 8.0),
                    _ => Point2::new(0.0, 8.0 - t * 8.0),
                });
            }
        }
        points
    }

    #[test]
    fn exact_overlap_covers_4_8_16_vertex_work_matrix() {
        let mut prior_operations = 0;
        for count in [4, 8, 16] {
            let subject = rectangle_with_vertices(count);
            let clip = [
                Point2::new(2.0, -1.0),
                Point2::new(6.0, -1.0),
                Point2::new(6.0, 9.0),
                Point2::new(2.0, 9.0),
            ];
            let overlap = exact_convex_polygon_overlap_v1(
                &subject,
                &clip,
                ExactConvexOverlapLimitsV1::default(),
            )
            .unwrap();
            assert!((4..=count + 4).contains(&overlap.points().len()));
            assert!(overlap.signed_double_area().is_positive());
            assert!(overlap.points().iter().all(|p| p.rounded().is_some()));
            assert!(overlap.operations() > prior_operations);
            prior_operations = overlap.operations();
        }
    }

    #[test]
    fn limits_fail_exactly_one_short() {
        let subject = rectangle_with_vertices(8);
        let clip = rectangle_with_vertices(4);
        let result = exact_convex_polygon_overlap_v1(&subject, &clip, Default::default()).unwrap();
        for limits in [
            ExactConvexOverlapLimitsV1 {
                max_operations: result.operations() - 1,
                ..Default::default()
            },
            ExactConvexOverlapLimitsV1 {
                max_bit_work: result.bit_work() - 1,
                ..Default::default()
            },
            ExactConvexOverlapLimitsV1 {
                max_integer_bits: result.maximum_integer_bits() - 1,
                ..Default::default()
            },
            ExactConvexOverlapLimitsV1 {
                max_output_vertices: result.points().len() - 1,
                ..Default::default()
            },
        ] {
            assert_eq!(
                exact_convex_polygon_overlap_v1(&subject, &clip, limits),
                Err(ExactConvexOverlapErrorV1::ResourceLimit)
            );
        }
    }

    #[test]
    fn winding_start_collinear_and_duplicate_contracts_are_deterministic() {
        let subject = rectangle_with_vertices(8);
        let clip = rectangle_with_vertices(4);
        let expected =
            exact_convex_polygon_overlap_v1(&subject, &clip, Default::default()).unwrap();
        let mut reordered = subject.clone();
        reordered.rotate_left(3);
        reordered.reverse();
        let actual =
            exact_convex_polygon_overlap_v1(&reordered, &clip, Default::default()).unwrap();
        assert_eq!(actual.points(), expected.points());
        assert_eq!(actual.signed_double_area(), expected.signed_double_area());

        let duplicate = [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 1.0),
        ];
        assert_eq!(
            exact_convex_polygon_overlap_v1(&duplicate, &clip, Default::default()),
            Err(ExactConvexOverlapErrorV1::InvalidInput)
        );

        let limits = ExactConvexOverlapLimitsV1 {
            max_operations: 1,
            ..Default::default()
        };
        let triangle = [
            Point2::new(0.0, 0.0),
            Point2::new(1.0, 0.0),
            Point2::new(0.0, 1.0),
        ];
        assert_eq!(
            exact_convex_polygon_overlap_v1(&triangle, &triangle, limits),
            Err(ExactConvexOverlapErrorV1::ResourceLimit)
        );
    }
}
