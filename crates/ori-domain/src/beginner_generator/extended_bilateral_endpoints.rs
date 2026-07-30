use crate::{BeginnerProtrusionSymmetryV1, BeginnerProtrusionTargetV1, BeginnerSkeletonSegmentV1};

/// Converts one bounded six- or eight-protrusion bilateral record into
/// canonical normalized paper endpoints.
///
/// This is proposal geometry only. It does not establish foldability or
/// authorize mutation. The authored position must lie on the vertical axis
/// of the skeleton bounds. Endpoints are emitted from the lowest pair to the
/// highest pair, with the left endpoint first in every pair.
pub(super) fn parameterized_extended_bilateral_endpoints_v1(
    target: &BeginnerProtrusionTargetV1,
    segments: &[BeginnerSkeletonSegmentV1],
) -> Option<Vec<(f64, f64)>> {
    if target.symmetry != BeginnerProtrusionSymmetryV1::Bilateral || !matches!(target.count, 6 | 8)
    {
        return None;
    }
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
    if span_x <= 0
        || span_y <= 0
        || target.position_tenths_mm[0].checked_mul(2)? != minimum_x.checked_add(maximum_x)?
        || !(minimum_y..=maximum_y).contains(&target.position_tenths_mm[1])
    {
        return None;
    }

    let vertical =
        target.direction_milli[1].unsigned_abs() > target.direction_milli[0].unsigned_abs();
    let primary_direction = if vertical {
        target.direction_milli[1]
    } else {
        target.direction_milli[0]
    };
    if primary_direction == 0 {
        return None;
    }
    let primary_span = if vertical { span_y } else { span_x };
    let radial_span = span_x.min(span_y);
    let length_ratio =
        f64::from(target.length_tenths_mm) / f64::from(u32::try_from(primary_span).ok()?);
    let root_width = target
        .root_width_tenths_mm
        .unwrap_or(u32::from(target.thickness_tenths_mm));
    let tip_width = target.tip_width_tenths_mm.unwrap_or(root_width);
    let width_ratio = f64::from(root_width.checked_add(tip_width)?)
        / 2.0
        / f64::from(u32::try_from(radial_span).ok()?);
    if !(0.02..=0.45).contains(&length_ratio) || !(0.001..=0.25).contains(&width_ratio) {
        return None;
    }

    let priority_scale = 0.75 + f64::from(target.priority) / 400.0;
    let direction_scale = f64::from(primary_direction.unsigned_abs()) / 1_000.0;
    let reach = length_ratio * priority_scale * direction_scale;
    let spread = (width_ratio * 2.0).clamp(0.05, 0.2);
    let center_y = f64::from(target.position_tenths_mm[1].checked_sub(minimum_y)?)
        / f64::from(u32::try_from(span_y).ok()?);
    let pair_count = usize::from(target.count / 2);

    let mut endpoints = Vec::new();
    endpoints
        .try_reserve_exact(usize::from(target.count))
        .ok()?;
    for pair_index in 0..pair_count {
        let interpolation = (pair_index as f64) / ((pair_count - 1) as f64);
        let pair = if vertical {
            let y = center_y - reach + 2.0 * reach * interpolation;
            [(0.5 - spread, y), (0.5 + spread, y)]
        } else {
            let y = center_y - spread + 2.0 * spread * interpolation;
            [(0.5 - reach, y), (0.5 + reach, y)]
        };
        if pair.iter().any(|endpoint| {
            !endpoint.0.is_finite()
                || !endpoint.1.is_finite()
                || !(0.0..1.0).contains(&endpoint.0)
                || !(0.0..1.0).contains(&endpoint.1)
                || endpoints.iter().any(|existing: &(f64, f64)| {
                    existing.0.to_bits() == endpoint.0.to_bits()
                        && existing.1.to_bits() == endpoint.1.to_bits()
                })
        }) {
            return None;
        }
        endpoints.extend(pair);
    }
    (endpoints.len() == usize::from(target.count)).then_some(endpoints)
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

    fn target(count: u8, direction: [i16; 3]) -> BeginnerProtrusionTargetV1 {
        BeginnerProtrusionTargetV1 {
            id: 1,
            count,
            length_tenths_mm: 100,
            thickness_tenths_mm: 10,
            root_width_tenths_mm: None,
            tip_width_tenths_mm: None,
            local_outline_tenths_mm: None,
            position_tenths_mm: [500, 500, 0],
            direction_milli: direction,
            symmetry: BeginnerProtrusionSymmetryV1::Bilateral,
            curvature_degrees: 0,
            joint: BeginnerProtrusionJointV1::Fixed,
            motion_degrees: [0, 0],
            side: BeginnerProtrusionSideV1::Either,
            priority: 100,
        }
    }

    #[test]
    fn six_and_eight_are_canonical_bilateral_and_segment_order_independent() {
        let segments = [
            segment(1, (0, 0), (1_000, 0)),
            segment(2, (1_000, 0), (1_000, 1_000)),
        ];
        for count in [6, 8] {
            for direction in [[1_000, 0, 0], [0, 1_000, 0]] {
                let endpoints = parameterized_extended_bilateral_endpoints_v1(
                    &target(count, direction),
                    &segments,
                )
                .expect("bounded bilateral family");
                assert_eq!(endpoints.len(), usize::from(count));
                assert_eq!(
                    endpoints
                        .iter()
                        .map(|point| (point.0.to_bits(), point.1.to_bits()))
                        .collect::<std::collections::HashSet<_>>()
                        .len(),
                    usize::from(count)
                );
                for pair in endpoints.chunks_exact(2) {
                    assert!((pair[0].0 + pair[1].0 - 1.0).abs() <= f64::EPSILON);
                    assert!((pair[0].1 - pair[1].1).abs() <= f64::EPSILON);
                }

                let mut reversed = segments;
                reversed.reverse();
                assert_eq!(
                    parameterized_extended_bilateral_endpoints_v1(
                        &target(count, direction),
                        &reversed,
                    ),
                    Some(endpoints)
                );
            }
        }
    }

    #[test]
    fn extended_bilateral_fails_closed_for_wrong_family_axis_direction_and_escape() {
        let segments = [
            segment(1, (0, 0), (1_000, 0)),
            segment(2, (1_000, 0), (1_000, 1_000)),
        ];
        let mut invalid = target(6, [1_000, 0, 0]);
        invalid.symmetry = BeginnerProtrusionSymmetryV1::Radial;
        assert!(parameterized_extended_bilateral_endpoints_v1(&invalid, &segments).is_none());

        invalid = target(4, [1_000, 0, 0]);
        assert!(parameterized_extended_bilateral_endpoints_v1(&invalid, &segments).is_none());

        invalid = target(6, [1_000, 0, 0]);
        invalid.position_tenths_mm[0] = 499;
        assert!(parameterized_extended_bilateral_endpoints_v1(&invalid, &segments).is_none());

        invalid = target(6, [0, 0, 1_000]);
        assert!(parameterized_extended_bilateral_endpoints_v1(&invalid, &segments).is_none());

        invalid = target(8, [0, 1_000, 0]);
        invalid.position_tenths_mm[1] = 950;
        invalid.length_tenths_mm = 450;
        assert!(parameterized_extended_bilateral_endpoints_v1(&invalid, &segments).is_none());
    }
}
