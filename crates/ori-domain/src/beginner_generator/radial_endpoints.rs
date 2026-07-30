use crate::{BeginnerProtrusionSymmetryV1, BeginnerProtrusionTargetV1, BeginnerSkeletonSegmentV1};

const TURN_SCALE_V1: i64 = 1_000_000;

// Canonical counter-clockwise rotations. Integer millionths keep candidate
// generation independent of platform trigonometric functions; these values
// are proposal geometry only and never become proof or mutation authority.
const RADIAL_ROTATIONS_2_V1: [[i32; 2]; 2] = [[1_000_000, 0], [-1_000_000, 0]];
const RADIAL_ROTATIONS_3_V1: [[i32; 2]; 3] =
    [[1_000_000, 0], [-500_000, 866_025], [-500_000, -866_025]];
const RADIAL_ROTATIONS_4_V1: [[i32; 2]; 4] = [
    [1_000_000, 0],
    [0, 1_000_000],
    [-1_000_000, 0],
    [0, -1_000_000],
];
const RADIAL_ROTATIONS_5_V1: [[i32; 2]; 5] = [
    [1_000_000, 0],
    [309_017, 951_057],
    [-809_017, 587_785],
    [-809_017, -587_785],
    [309_017, -951_057],
];
const RADIAL_ROTATIONS_6_V1: [[i32; 2]; 6] = [
    [1_000_000, 0],
    [500_000, 866_025],
    [-500_000, 866_025],
    [-1_000_000, 0],
    [-500_000, -866_025],
    [500_000, -866_025],
];
const RADIAL_ROTATIONS_7_V1: [[i32; 2]; 7] = [
    [1_000_000, 0],
    [623_490, 781_831],
    [-222_521, 974_928],
    [-900_969, 433_884],
    [-900_969, -433_884],
    [-222_521, -974_928],
    [623_490, -781_831],
];
const RADIAL_ROTATIONS_8_V1: [[i32; 2]; 8] = [
    [1_000_000, 0],
    [707_107, 707_107],
    [0, 1_000_000],
    [-707_107, 707_107],
    [-1_000_000, 0],
    [-707_107, -707_107],
    [0, -1_000_000],
    [707_107, -707_107],
];

fn radial_rotations_v1(count: u8) -> Option<&'static [[i32; 2]]> {
    match count {
        2 => Some(&RADIAL_ROTATIONS_2_V1),
        3 => Some(&RADIAL_ROTATIONS_3_V1),
        4 => Some(&RADIAL_ROTATIONS_4_V1),
        5 => Some(&RADIAL_ROTATIONS_5_V1),
        6 => Some(&RADIAL_ROTATIONS_6_V1),
        7 => Some(&RADIAL_ROTATIONS_7_V1),
        8 => Some(&RADIAL_ROTATIONS_8_V1),
        _ => None,
    }
}

/// Converts one bounded radial protrusion record into canonical normalized
/// paper endpoints.
///
/// The first endpoint follows the authored projected direction. Remaining
/// endpoints rotate counter-clockwise in record-local order. A Z-only
/// direction, an out-of-bounds endpoint, or a non-radial/count mismatch fails
/// closed so callers cannot silently collapse distinct protrusions.
pub(super) fn parameterized_radial_endpoints_v1(
    target: &BeginnerProtrusionTargetV1,
    segments: &[BeginnerSkeletonSegmentV1],
) -> Option<Vec<(f64, f64)>> {
    if target.symmetry != BeginnerProtrusionSymmetryV1::Radial {
        return None;
    }
    let rotations = radial_rotations_v1(target.count)?;
    let minimum_x = segments
        .iter()
        .flat_map(|segment| [segment.start.x_tenths_mm, segment.end.x_tenths_mm])
        .min()?;
    let maximum_x = segments
        .iter()
        .flat_map(|segment| [segment.start.x_tenths_mm, segment.end.x_tenths_mm])
        .max()?;
    let minimum_y = segments
        .iter()
        .flat_map(|segment| [segment.start.y_tenths_mm, segment.end.y_tenths_mm])
        .min()?;
    let maximum_y = segments
        .iter()
        .flat_map(|segment| [segment.start.y_tenths_mm, segment.end.y_tenths_mm])
        .max()?;
    let span_x = maximum_x.checked_sub(minimum_x)?;
    let span_y = maximum_y.checked_sub(minimum_y)?;
    let radial_span = u32::try_from(span_x.min(span_y)).ok()?;
    if radial_span == 0 {
        return None;
    }
    let center_x = f64::from(target.position_tenths_mm[0].checked_sub(minimum_x)?)
        / f64::from(u32::try_from(span_x).ok()?);
    let center_y = f64::from(target.position_tenths_mm[1].checked_sub(minimum_y)?)
        / f64::from(u32::try_from(span_y).ok()?);
    if !(0.0..=1.0).contains(&center_x) || !(0.0..=1.0).contains(&center_y) {
        return None;
    }

    let direction_x = i64::from(target.direction_milli[0]);
    let direction_y = i64::from(target.direction_milli[1]);
    let direction_squared = direction_x
        .checked_mul(direction_x)?
        .checked_add(direction_y.checked_mul(direction_y)?)?;
    let direction_norm = u64::try_from(direction_squared).ok()?.isqrt();
    if direction_norm == 0 {
        return None;
    }
    let length_ratio = f64::from(target.length_tenths_mm) / f64::from(radial_span);
    if !(0.02..=0.45).contains(&length_ratio) {
        return None;
    }
    let direction_strength = f64::from(
        target.direction_milli[0]
            .unsigned_abs()
            .max(target.direction_milli[1].unsigned_abs()),
    ) / 1_000.0;
    let reach = length_ratio * (0.75 + f64::from(target.priority) / 400.0) * direction_strength;
    let denominator = (direction_norm as f64) * (TURN_SCALE_V1 as f64);

    let mut endpoints = Vec::new();
    endpoints.try_reserve_exact(rotations.len()).ok()?;
    for [cos, sin] in rotations {
        let cos = i64::from(*cos);
        let sin = i64::from(*sin);
        let rotated_x = direction_x
            .checked_mul(cos)?
            .checked_sub(direction_y.checked_mul(sin)?)?;
        let rotated_y = direction_x
            .checked_mul(sin)?
            .checked_add(direction_y.checked_mul(cos)?)?;
        let endpoint = (
            center_x + reach * (rotated_x as f64) / denominator,
            center_y + reach * (rotated_y as f64) / denominator,
        );
        if !endpoint.0.is_finite()
            || !endpoint.1.is_finite()
            || !(0.0..1.0).contains(&endpoint.0)
            || !(0.0..1.0).contains(&endpoint.1)
            || endpoints.iter().any(|existing: &(f64, f64)| {
                existing.0.to_bits() == endpoint.0.to_bits()
                    && existing.1.to_bits() == endpoint.1.to_bits()
            })
        {
            return None;
        }
        endpoints.push(endpoint);
    }
    Some(endpoints)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BeginnerProtrusionJointV1, BeginnerProtrusionSideV1, BeginnerSkeletonPointV1};

    fn segment(id: u16, start: (i32, i32), end: (i32, i32)) -> BeginnerSkeletonSegmentV1 {
        BeginnerSkeletonSegmentV1 {
            id,
            start: BeginnerSkeletonPointV1 {
                x_tenths_mm: start.0,
                y_tenths_mm: start.1,
            },
            end: BeginnerSkeletonPointV1 {
                x_tenths_mm: end.0,
                y_tenths_mm: end.1,
            },
            thickness_tenths_mm: 10,
        }
    }

    fn target(count: u8) -> BeginnerProtrusionTargetV1 {
        BeginnerProtrusionTargetV1 {
            id: 1,
            count,
            length_tenths_mm: 100,
            thickness_tenths_mm: 10,
            root_width_tenths_mm: None,
            tip_width_tenths_mm: None,
            local_outline_tenths_mm: None,
            position_tenths_mm: [500, 500, 0],
            direction_milli: [1_000, 0, 0],
            symmetry: BeginnerProtrusionSymmetryV1::Radial,
            curvature_degrees: 0,
            joint: BeginnerProtrusionJointV1::Fixed,
            motion_degrees: [0, 0],
            side: BeginnerProtrusionSideV1::Either,
            priority: 100,
        }
    }

    #[test]
    fn every_bounded_radial_arity_is_canonical_distinct_and_order_independent() {
        let segments = [
            segment(1, (0, 0), (1_000, 0)),
            segment(2, (1_000, 0), (1_000, 1_000)),
        ];
        for count in 2..=8 {
            let endpoints = parameterized_radial_endpoints_v1(&target(count), &segments)
                .expect("radial family");
            assert_eq!(endpoints.len(), usize::from(count));
            assert!((endpoints[0].0 - 0.6).abs() <= f64::EPSILON);
            assert!((endpoints[0].1 - 0.5).abs() <= f64::EPSILON);
            assert_eq!(
                endpoints
                    .iter()
                    .map(|(x, y)| (x.to_bits(), y.to_bits()))
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                usize::from(count)
            );

            let mut reversed = segments;
            reversed.reverse();
            assert_eq!(
                parameterized_radial_endpoints_v1(&target(count), &reversed),
                Some(endpoints)
            );
        }
    }

    #[test]
    fn radial_endpoints_fail_closed_for_wrong_symmetry_z_only_and_paper_escape() {
        let segments = [
            segment(1, (0, 0), (1_000, 0)),
            segment(2, (1_000, 0), (1_000, 1_000)),
        ];
        let mut invalid = target(3);
        invalid.symmetry = BeginnerProtrusionSymmetryV1::Bilateral;
        assert!(parameterized_radial_endpoints_v1(&invalid, &segments).is_none());

        invalid = target(3);
        invalid.direction_milli = [0, 0, 1_000];
        assert!(parameterized_radial_endpoints_v1(&invalid, &segments).is_none());

        invalid = target(3);
        invalid.position_tenths_mm = [0, 500, 0];
        invalid.direction_milli = [-1_000, 0, 0];
        invalid.length_tenths_mm = 450;
        assert!(parameterized_radial_endpoints_v1(&invalid, &segments).is_none());

        assert!(parameterized_radial_endpoints_v1(&target(1), &segments).is_none());
    }
}
