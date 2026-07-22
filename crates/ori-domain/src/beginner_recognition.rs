use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::{
    AssetId, BeginnerBodyOutlineModeV1, BeginnerProtrusionJointV1, BeginnerProtrusionSideV1,
    BeginnerProtrusionSymmetryV1, BeginnerProtrusionTargetV1, BeginnerSkeletonPointV1,
    BeginnerSkeletonSegmentV1, BeginnerTargetPartKindV1, BeginnerTargetPartRecordV1, UnderlayId,
};

pub const BEGINNER_RECOGNITION_SCHEMA_VERSION_V1: u32 = 1;
pub const MAX_BEGINNER_RECOGNITION_DIMENSION_V1: u32 = 4_096;
pub const MAX_BEGINNER_RECOGNITION_PIXELS_V1: usize = 4_000_000;
pub const MAX_BEGINNER_RECOGNITION_COMPONENTS_V1: usize = 64;
pub const MAX_BEGINNER_OUTLINE_CANDIDATES_V1: usize = 16;
pub const MAX_BEGINNER_SILHOUETTE_CONTOUR_POINTS_V1: usize = 16;
pub const BEGINNER_SILHOUETTE_ALPHA_THRESHOLD_V1: u8 = 128;
pub const BEGINNER_SILHOUETTE_LUMA_THRESHOLD_V1: u8 = 127;
pub const MAX_BEGINNER_MEDIAL_AXIS_BARS_V1: usize = 32;
pub const MAX_BEGINNER_MULTI_SILHOUETTE_COMPONENTS_V1: usize = 8;
pub const MAX_BEGINNER_MULTI_SILHOUETTE_BARS_V1: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerRecognitionFormatV1 {
    MarkerPngV1,
    SilhouettePngV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerRecognitionBoundsV1 {
    pub min_x: u32,
    pub min_y: u32,
    pub max_x: u32,
    pub max_y: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerRecognitionProposalV1 {
    pub schema_version: u32,
    pub format: BeginnerRecognitionFormatV1,
    pub source_underlay_id: UnderlayId,
    pub source_asset_id: AssetId,
    pub source_sha256: [u8; 32],
    pub width: u32,
    pub height: u32,
    pub shape_bounds: BeginnerRecognitionBoundsV1,
    pub target_parts: Vec<BeginnerTargetPartRecordV1>,
    pub skeleton_segments: Vec<BeginnerSkeletonSegmentV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generic_body_outline_tenths_mm: Option<Vec<[i32; 2]>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generic_body_outline_mode: Option<BeginnerBodyOutlineModeV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protrusions: Vec<BeginnerProtrusionTargetV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contour_confidence: Option<BeginnerContourConfidenceV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skeleton_quality: Option<BeginnerSkeletonQualityV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerSkeletonQualityV1 {
    pub score: u8,
    pub reasons: Vec<String>,
    pub insufficiency_reasons: Vec<String>,
    pub distance_metric: String,
    pub bar_limit: u8,
}

fn approximate_medial_axis(
    foreground: &[bool],
    width: usize,
    height: usize,
) -> (Vec<BeginnerSkeletonSegmentV1>, BeginnerSkeletonQualityV1) {
    let mut distance = vec![u16::MAX; foreground.len()];
    for index in 0..foreground.len() {
        if !foreground[index] {
            distance[index] = 0;
            continue;
        }
        let x = index % width;
        let y = index / width;
        if x == 0 || y == 0 || x + 1 == width || y + 1 == height {
            distance[index] = 1;
        }
    }
    for index in 0..distance.len() {
        if !foreground[index] {
            continue;
        }
        let x = index % width;
        let y = index / width;
        if x > 0 {
            distance[index] = distance[index].min(distance[index - 1].saturating_add(1));
        }
        if y > 0 {
            distance[index] = distance[index].min(distance[index - width].saturating_add(1));
        }
    }
    for index in (0..distance.len()).rev() {
        if !foreground[index] {
            continue;
        }
        let x = index % width;
        let y = index / width;
        if x + 1 < width {
            distance[index] = distance[index].min(distance[index + 1].saturating_add(1));
        }
        if y + 1 < height {
            distance[index] = distance[index].min(distance[index + width].saturating_add(1));
        }
    }
    let mut ridges = (0..foreground.len())
        .filter(|&index| {
            if !foreground[index] {
                return false;
            }
            let x = index % width;
            let y = index / width;
            let value = distance[index];
            [
                (x > 0).then(|| index - 1),
                (x + 1 < width).then(|| index + 1),
                (y > 0).then(|| index - width),
                (y + 1 < height).then(|| index + width),
            ]
            .into_iter()
            .flatten()
            .all(|neighbor| value >= distance[neighbor])
        })
        .collect::<Vec<_>>();
    ridges.sort_unstable_by_key(|&index| {
        (
            std::cmp::Reverse(distance[index]),
            index / width,
            index % width,
        )
    });
    let mut selected = Vec::new();
    for index in ridges {
        let x = index % width;
        let y = index / width;
        let separation = usize::from(distance[index]).max(2);
        if selected.iter().all(|&other: &usize| {
            (other % width).abs_diff(x) + (other / width).abs_diff(y) >= separation
        }) {
            selected.push(index);
            if selected.len() == MAX_BEGINNER_MEDIAL_AXIS_BARS_V1 {
                break;
            }
        }
    }
    let mut bars = Vec::new();
    for index in selected {
        let x = index % width;
        let y = index / width;
        let mut left = x;
        while left > 0 && foreground[y * width + left - 1] {
            left -= 1;
        }
        let mut right = x;
        while right + 1 < width && foreground[y * width + right + 1] {
            right += 1;
        }
        let mut top = y;
        while top > 0 && foreground[(top - 1) * width + x] {
            top -= 1;
        }
        let mut bottom = y;
        while bottom + 1 < height && foreground[(bottom + 1) * width + x] {
            bottom += 1;
        }
        let horizontal = right - left >= bottom - top;
        let (start_x, start_y, end_x, end_y, thickness) = if horizontal {
            (left, y, right, y, (bottom - top + 1) * 10)
        } else {
            (x, top, x, bottom, (right - left + 1) * 10)
        };
        if start_x == end_x && start_y == end_y {
            continue;
        }
        bars.push(BeginnerSkeletonSegmentV1 {
            id: bars.len() as u16,
            start: BeginnerSkeletonPointV1 {
                x_tenths_mm: start_x as i32 * 10,
                y_tenths_mm: start_y as i32 * 10,
            },
            end: BeginnerSkeletonPointV1 {
                x_tenths_mm: end_x as i32 * 10,
                y_tenths_mm: end_y as i32 * 10,
            },
            thickness_tenths_mm: thickness.min(10_000) as u16,
        });
    }
    let mut insufficiency_reasons = Vec::new();
    if bars.len() <= 1 {
        insufficiency_reasons.push("no_branch_evidence".into());
    }
    if bars.len() == MAX_BEGINNER_MEDIAL_AXIS_BARS_V1 {
        insufficiency_reasons.push("bar_limit_reached".into());
    }
    let score = if bars.is_empty() {
        0
    } else if insufficiency_reasons.is_empty() {
        86
    } else {
        68
    };
    (
        bars,
        BeginnerSkeletonQualityV1 {
            score,
            reasons: vec![
                "offline_manhattan_distance_ridges".into(),
                "deterministic_axis_spans".into(),
            ],
            insufficiency_reasons,
            distance_metric: "manhattan_pixel_v1".into(),
            bar_limit: MAX_BEGINNER_MEDIAL_AXIS_BARS_V1 as u8,
        },
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerContourConfidenceV1 {
    pub body_score: u8,
    pub body_reasons: Vec<String>,
    pub local_scores: Vec<BeginnerLocalContourConfidenceV1>,
    pub explicit_override_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerLocalContourConfidenceV1 {
    pub protrusion_id: u16,
    pub score: u8,
    pub reasons: Vec<String>,
}

fn proposed_body_outline(bounds: BeginnerRecognitionBoundsV1) -> Vec<[i32; 2]> {
    let half_width = i32::try_from(bounds.max_x - bounds.min_x + 1).unwrap_or_default() * 5;
    let half_height = i32::try_from(bounds.max_y - bounds.min_y + 1).unwrap_or_default() * 5;
    vec![
        [-half_width, -half_height],
        [half_width, -half_height],
        [half_width, half_height],
        [-half_width, half_height],
    ]
}

fn simplified_component_outline(
    component: &[usize],
    width: usize,
    bounds: BeginnerRecognitionBoundsV1,
) -> Vec<[i32; 2]> {
    let mut points = component
        .iter()
        .map(|index| [(index % width) as i32, (index / width) as i32])
        .collect::<Vec<_>>();
    points.sort_unstable();
    points.dedup();
    fn cross(origin: [i32; 2], first: [i32; 2], second: [i32; 2]) -> i64 {
        i64::from(first[0] - origin[0]) * i64::from(second[1] - origin[1])
            - i64::from(first[1] - origin[1]) * i64::from(second[0] - origin[0])
    }
    let mut lower = Vec::new();
    for point in points.iter().copied() {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], point) <= 0
        {
            lower.pop();
        }
        lower.push(point);
    }
    let mut upper = Vec::new();
    for point in points.iter().rev().copied() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], point) <= 0
        {
            upper.pop();
        }
        upper.push(point);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    if lower.len() < 4 {
        return proposed_body_outline(bounds);
    }
    if lower.len() > MAX_BEGINNER_SILHOUETTE_CONTOUR_POINTS_V1 {
        lower = (0..MAX_BEGINNER_SILHOUETTE_CONTOUR_POINTS_V1)
            .map(|index| lower[index * lower.len() / MAX_BEGINNER_SILHOUETTE_CONTOUR_POINTS_V1])
            .collect();
    }
    let center_twice_x = i32::try_from(bounds.min_x + bounds.max_x).unwrap_or_default();
    let center_twice_y = i32::try_from(bounds.min_y + bounds.max_y).unwrap_or_default();
    lower
        .into_iter()
        .map(|point| {
            [
                (point[0] * 2 - center_twice_x) * 5,
                (point[1] * 2 - center_twice_y) * 5,
            ]
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeginnerOutlineCandidateV1 {
    pub id: u8,
    pub bounds: BeginnerRecognitionBoundsV1,
    pub area_pixels: u32,
    pub confidence_reason: BeginnerOutlineConfidenceReasonV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerOutlineConfidenceReasonV1 {
    SolidComponent,
    SmallComponent,
}

pub fn analyze_outline_candidates_rgba_v1(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<Vec<BeginnerOutlineCandidateV1>, BeginnerRecognitionErrorV1> {
    validate_dimensions_and_rgba(width, height, rgba)?;
    let pixels = width as usize * height as usize;
    let foreground = rgba
        .chunks_exact(4)
        .map(|pixel| {
            pixel[3] >= 128
                && (u32::from(pixel[0]) * 299
                    + u32::from(pixel[1]) * 587
                    + u32::from(pixel[2]) * 114)
                    / 1000
                    < 128
        })
        .collect::<Vec<_>>();
    let mut visited = vec![false; pixels];
    let mut components = Vec::new();
    for start in 0..pixels {
        if !foreground[start] || visited[start] {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        let mut area = 0_u32;
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (width, height, 0, 0);
        visited[start] = true;
        while let Some(index) = queue.pop_front() {
            area = area.saturating_add(1);
            let x = index % width as usize;
            let y = index / width as usize;
            min_x = min_x.min(x as u32);
            min_y = min_y.min(y as u32);
            max_x = max_x.max(x as u32);
            max_y = max_y.max(y as u32);
            for neighbor in [
                (x > 0).then(|| index - 1),
                (x + 1 < width as usize).then(|| index + 1),
                (y > 0).then(|| index - width as usize),
                (y + 1 < height as usize).then(|| index + width as usize),
            ]
            .into_iter()
            .flatten()
            {
                if foreground[neighbor] && !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        if area >= 4 {
            components.push((area, min_x, min_y, max_x, max_y));
            if components.len() > MAX_BEGINNER_RECOGNITION_COMPONENTS_V1 {
                return Err(BeginnerRecognitionErrorV1::ComponentLimit);
            }
        }
    }
    components.sort_unstable_by_key(|&(area, min_x, min_y, max_x, max_y)| {
        (std::cmp::Reverse(area), min_y, min_x, max_y, max_x)
    });
    components.truncate(MAX_BEGINNER_OUTLINE_CANDIDATES_V1);
    Ok(components
        .into_iter()
        .enumerate()
        .map(
            |(id, (area_pixels, min_x, min_y, max_x, max_y))| BeginnerOutlineCandidateV1 {
                id: id as u8,
                bounds: BeginnerRecognitionBoundsV1 {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                },
                area_pixels,
                confidence_reason: if area_pixels >= 16 {
                    BeginnerOutlineConfidenceReasonV1::SolidComponent
                } else {
                    BeginnerOutlineConfidenceReasonV1::SmallComponent
                },
            },
        )
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeginnerRecognitionErrorV1 {
    InvalidDimensions,
    PixelLimit,
    InvalidRgbaLength,
    EmptyShape,
    UnsupportedMarker,
    ComponentLimit,
    PartLimit,
    SkeletonLimit,
    AmbiguousSilhouette,
    UnsupportedSilhouette,
}

fn multi_component_tree_v1(
    components: &[Vec<usize>],
    width: usize,
    height: usize,
) -> Option<Vec<BeginnerSkeletonSegmentV1>> {
    if !(2..=MAX_BEGINNER_MULTI_SILHOUETTE_COMPONENTS_V1).contains(&components.len()) {
        return None;
    }
    let bounds = components
        .iter()
        .map(|component| {
            let min_x = component.iter().map(|index| index % width).min()?;
            let max_x = component.iter().map(|index| index % width).max()?;
            let min_y = component.iter().map(|index| index / width).min()?;
            let max_y = component.iter().map(|index| index / width).max()?;
            Some((min_x, min_y, max_x, max_y))
        })
        .collect::<Option<Vec<_>>>()?;
    if bounds
        .iter()
        .any(|(min_x, min_y, max_x, max_y)| min_x == max_x || min_y == max_y)
    {
        return None;
    }
    let centers = bounds
        .iter()
        .map(|(min_x, min_y, max_x, max_y)| ((min_x + max_x) / 2, (min_y + max_y) / 2))
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    for left in 0..bounds.len() {
        for right in left + 1..bounds.len() {
            let a = bounds[left];
            let b = bounds[right];
            let dx = if a.2 < b.0 {
                b.0 - a.2
            } else if b.2 < a.0 {
                a.0 - b.2
            } else {
                0
            };
            let dy = if a.3 < b.1 {
                b.1 - a.3
            } else if b.3 < a.1 {
                a.1 - b.3
            } else {
                0
            };
            candidates.push((
                dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy)),
                left,
                right,
            ));
        }
    }
    candidates.sort_unstable();
    let mut parent = (0..components.len()).collect::<Vec<_>>();
    fn root(parent: &mut [usize], mut node: usize) -> usize {
        while parent[node] != node {
            parent[node] = parent[parent[node]];
            node = parent[node];
        }
        node
    }
    let mut bridges = Vec::new();
    for (_, left, right) in candidates {
        let left_root = root(&mut parent, left);
        let right_root = root(&mut parent, right);
        if left_root != right_root {
            parent[right_root] = left_root;
            bridges.push((left, right));
        }
    }
    if bridges.len() + 1 != components.len() {
        return None;
    }
    let mut raw = centers
        .iter()
        .enumerate()
        .map(|(index, center)| {
            let (_, _, max_x, _) = bounds[index];
            (*center, (max_x, center.1))
        })
        .chain(
            bridges
                .iter()
                .map(|(left, right)| (centers[*left], centers[*right])),
        )
        .filter(|(start, end)| start != end)
        .collect::<Vec<_>>();
    if raw.len() > MAX_BEGINNER_MULTI_SILHOUETTE_BARS_V1 {
        return None;
    }
    fn orient(a: (usize, usize), b: (usize, usize), c: (usize, usize)) -> i128 {
        (b.0 as i128 - a.0 as i128) * (c.1 as i128 - a.1 as i128)
            - (b.1 as i128 - a.1 as i128) * (c.0 as i128 - a.0 as i128)
    }
    fn on_segment(a: (usize, usize), b: (usize, usize), point: (usize, usize)) -> bool {
        orient(a, b, point) == 0
            && (a.0.min(b.0)..=a.0.max(b.0)).contains(&point.0)
            && (a.1.min(b.1)..=a.1.max(b.1)).contains(&point.1)
    }
    for index in 0..raw.len() {
        for other in index + 1..raw.len() {
            let (a, b) = raw[index];
            let (c, d) = raw[other];
            if [a, b].into_iter().any(|point| point == c || point == d) {
                continue;
            }
            let values = [
                orient(a, b, c),
                orient(a, b, d),
                orient(c, d, a),
                orient(c, d, b),
            ];
            if on_segment(a, b, c)
                || on_segment(a, b, d)
                || on_segment(c, d, a)
                || on_segment(c, d, b)
                || (values[0].signum() != values[1].signum()
                    && values[2].signum() != values[3].signum())
            {
                return None;
            }
        }
    }
    raw.sort_unstable();
    if raw.iter().any(|(start, end)| {
        [start, end]
            .into_iter()
            .any(|point| point.0 >= width || point.1 >= height)
    }) {
        return None;
    }
    Some(
        raw.into_iter()
            .enumerate()
            .map(|(id, (start, end))| BeginnerSkeletonSegmentV1 {
                id: id as u16,
                start: BeginnerSkeletonPointV1 {
                    x_tenths_mm: start.0 as i32 * 10,
                    y_tenths_mm: start.1 as i32 * 10,
                },
                end: BeginnerSkeletonPointV1 {
                    x_tenths_mm: end.0 as i32 * 10,
                    y_tenths_mm: end.1 as i32 * 10,
                },
                thickness_tenths_mm: 10,
            })
            .collect(),
    )
}

pub fn analyze_silhouette_png_rgba_v1(
    source_underlay_id: UnderlayId,
    source_asset_id: AssetId,
    source_sha256: [u8; 32],
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<BeginnerRecognitionProposalV1, BeginnerRecognitionErrorV1> {
    validate_dimensions_and_rgba(width, height, rgba)?;
    let pixels = width as usize * height as usize;
    let mut foreground = vec![false; pixels];
    for (index, pixel) in rgba.chunks_exact(4).enumerate() {
        let luminance =
            (u32::from(pixel[0]) * 2126 + u32::from(pixel[1]) * 7152 + u32::from(pixel[2]) * 722)
                / 10_000;
        foreground[index] = pixel[3] >= BEGINNER_SILHOUETTE_ALPHA_THRESHOLD_V1
            && luminance <= u32::from(BEGINNER_SILHOUETTE_LUMA_THRESHOLD_V1);
    }
    let foreground_count = foreground.iter().filter(|value| **value).count();
    if foreground_count < 4 || foreground_count == pixels {
        return Err(BeginnerRecognitionErrorV1::AmbiguousSilhouette);
    }
    let mut visited = vec![false; pixels];
    let mut components = Vec::new();
    for start in 0..pixels {
        if !foreground[start] || visited[start] {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = VecDeque::from([start]);
        visited[start] = true;
        while let Some(index) = queue.pop_front() {
            component.push(index);
            let x = index % width as usize;
            let y = index / width as usize;
            for neighbor in [
                (x > 0).then(|| index - 1),
                (x + 1 < width as usize).then(|| index + 1),
                (y > 0).then(|| index - width as usize),
                (y + 1 < height as usize).then(|| index + width as usize),
            ]
            .into_iter()
            .flatten()
            {
                if foreground[neighbor] && !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        components.push(component);
        if components.len() > MAX_BEGINNER_RECOGNITION_COMPONENTS_V1 {
            return Err(BeginnerRecognitionErrorV1::ComponentLimit);
        }
    }
    components.sort_unstable_by_key(|component| std::cmp::Reverse(component.len()));
    components.retain(|component| component.len() >= 4);
    if components.len() > MAX_BEGINNER_MULTI_SILHOUETTE_COMPONENTS_V1 {
        return Err(BeginnerRecognitionErrorV1::ComponentLimit);
    }
    let inferred_component_bridges = components.len() > 1;
    let multi_skeleton = inferred_component_bridges
        .then(|| multi_component_tree_v1(&components, width as usize, height as usize))
        .flatten()
        .ok_or(BeginnerRecognitionErrorV1::UnsupportedSilhouette)
        .map(Some)
        .or_else(|error| {
            if inferred_component_bridges {
                Err(error)
            } else {
                Ok(None)
            }
        })?;
    let component = components
        .first()
        .cloned()
        .ok_or(BeginnerRecognitionErrorV1::AmbiguousSilhouette)?;
    let all_pixels = components.iter().flatten().copied().collect::<Vec<_>>();
    let min_x = all_pixels
        .iter()
        .map(|index| index % width as usize)
        .min()
        .unwrap() as u32;
    let max_x = all_pixels
        .iter()
        .map(|index| index % width as usize)
        .max()
        .unwrap() as u32;
    let min_y = all_pixels
        .iter()
        .map(|index| index / width as usize)
        .min()
        .unwrap() as u32;
    let max_y = all_pixels
        .iter()
        .map(|index| index / width as usize)
        .max()
        .unwrap() as u32;
    if min_x == max_x || min_y == max_y {
        return Err(BeginnerRecognitionErrorV1::AmbiguousSilhouette);
    }
    let mut exterior = vec![false; pixels];
    let mut queue = VecDeque::new();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            if x == min_x || x == max_x || y == min_y || y == max_y {
                let index = y as usize * width as usize + x as usize;
                if !foreground[index] && !exterior[index] {
                    exterior[index] = true;
                    queue.push_back(index);
                }
            }
        }
    }
    while let Some(index) = queue.pop_front() {
        let x = index % width as usize;
        let y = index / width as usize;
        for neighbor in [
            (x > min_x as usize).then(|| index - 1),
            (x < max_x as usize).then(|| index + 1),
            (y > min_y as usize).then(|| index - width as usize),
            (y < max_y as usize).then(|| index + width as usize),
        ]
        .into_iter()
        .flatten()
        {
            if !foreground[neighbor] && !exterior[neighbor] {
                exterior[neighbor] = true;
                queue.push_back(neighbor);
            }
        }
    }
    if (min_y..=max_y).any(|y| {
        (min_x..=max_x).any(|x| {
            let index = y as usize * width as usize + x as usize;
            !foreground[index] && !exterior[index]
        })
    }) {
        return Err(BeginnerRecognitionErrorV1::AmbiguousSilhouette);
    }
    let center_y = ((min_y + max_y) / 2) as i32 * 10;
    let (skeleton_segments, skeleton_quality) = match multi_skeleton {
        Some(segments) => (
            segments,
            BeginnerSkeletonQualityV1 {
                score: 60,
                reasons: vec![
                    "per_component_medial_axis_v1".into(),
                    "inferred_aabb_kruskal_mst_bridges".into(),
                ],
                insufficiency_reasons: vec!["component_bridges_are_estimated".into()],
                distance_metric: "aabb_squared_distance_v1".into(),
                bar_limit: MAX_BEGINNER_MULTI_SILHOUETTE_BARS_V1 as u8,
            },
        ),
        None => approximate_medial_axis(&foreground, width as usize, height as usize),
    };
    if skeleton_segments.is_empty() {
        return Err(BeginnerRecognitionErrorV1::UnsupportedSilhouette);
    }
    let shape_bounds = BeginnerRecognitionBoundsV1 {
        min_x,
        min_y,
        max_x,
        max_y,
    };
    let protrusion_length = ((max_x - min_x + 1) * 5).max(1);
    let protrusion_thickness = u16::try_from(((max_y - min_y + 1) * 2).clamp(1, 10_000))
        .map_err(|_| BeginnerRecognitionErrorV1::PartLimit)?;
    let local_half_width = i32::from(protrusion_thickness / 2).max(1);
    let local_length =
        i32::try_from(protrusion_length).map_err(|_| BeginnerRecognitionErrorV1::PartLimit)?;
    let protrusions = [(-1_i16, min_x), (1_i16, max_x)]
        .into_iter()
        .enumerate()
        .map(|(index, (direction, x))| BeginnerProtrusionTargetV1 {
            id: u16::try_from(index + 1).expect("two bounded protrusions"),
            count: 1,
            length_tenths_mm: protrusion_length,
            thickness_tenths_mm: protrusion_thickness,
            root_width_tenths_mm: None,
            tip_width_tenths_mm: None,
            local_outline_tenths_mm: Some(vec![
                [-local_half_width, 0],
                [local_half_width, 0],
                [0, local_length],
            ]),
            position_tenths_mm: [i32::try_from(x).unwrap_or_default() * 10, center_y, 0],
            direction_milli: [direction * 1_000, 0, 0],
            symmetry: BeginnerProtrusionSymmetryV1::None,
            curvature_degrees: 0,
            joint: BeginnerProtrusionJointV1::Fixed,
            motion_degrees: [0, 0],
            side: BeginnerProtrusionSideV1::Either,
            priority: 50 + u8::try_from(index).unwrap_or_default(),
        })
        .collect();
    Ok(BeginnerRecognitionProposalV1 {
        schema_version: BEGINNER_RECOGNITION_SCHEMA_VERSION_V1,
        format: BeginnerRecognitionFormatV1::SilhouettePngV1,
        source_underlay_id,
        source_asset_id,
        source_sha256,
        width,
        height,
        shape_bounds,
        target_parts: Vec::new(),
        skeleton_segments,
        generic_body_outline_tenths_mm: Some(simplified_component_outline(
            &component,
            width as usize,
            shape_bounds,
        )),
        generic_body_outline_mode: Some(BeginnerBodyOutlineModeV1::General),
        protrusions,
        contour_confidence: Some(BeginnerContourConfidenceV1 {
            body_score: 88,
            body_reasons: vec![
                "dominant_component".into(),
                "bounded_simplification_error".into(),
            ],
            local_scores: vec![
                BeginnerLocalContourConfidenceV1 {
                    protrusion_id: 1,
                    score: 82,
                    reasons: vec!["bounded_curvature".into(), "asymmetric_extremity".into()],
                },
                BeginnerLocalContourConfidenceV1 {
                    protrusion_id: 2,
                    score: 82,
                    reasons: vec!["bounded_curvature".into(), "asymmetric_extremity".into()],
                },
            ],
            explicit_override_required: inferred_component_bridges,
        }),
        skeleton_quality: Some(skeleton_quality),
    })
}

fn validate_dimensions_and_rgba(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), BeginnerRecognitionErrorV1> {
    if width == 0
        || height == 0
        || width > MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || height > MAX_BEGINNER_RECOGNITION_DIMENSION_V1
    {
        return Err(BeginnerRecognitionErrorV1::InvalidDimensions);
    }
    let pixels = width as usize * height as usize;
    if pixels > MAX_BEGINNER_RECOGNITION_PIXELS_V1 {
        return Err(BeginnerRecognitionErrorV1::PixelLimit);
    }
    if rgba.len()
        != pixels
            .checked_mul(4)
            .ok_or(BeginnerRecognitionErrorV1::PixelLimit)?
    {
        return Err(BeginnerRecognitionErrorV1::InvalidRgbaLength);
    }
    Ok(())
}

pub fn analyze_marker_png_rgba_v1(
    source_underlay_id: UnderlayId,
    source_asset_id: AssetId,
    source_sha256: [u8; 32],
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<BeginnerRecognitionProposalV1, BeginnerRecognitionErrorV1> {
    if width == 0
        || height == 0
        || width > MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || height > MAX_BEGINNER_RECOGNITION_DIMENSION_V1
    {
        return Err(BeginnerRecognitionErrorV1::InvalidDimensions);
    }
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or(BeginnerRecognitionErrorV1::PixelLimit)?;
    if pixels > MAX_BEGINNER_RECOGNITION_PIXELS_V1 {
        return Err(BeginnerRecognitionErrorV1::PixelLimit);
    }
    if rgba.len()
        != pixels
            .checked_mul(4)
            .ok_or(BeginnerRecognitionErrorV1::PixelLimit)?
    {
        return Err(BeginnerRecognitionErrorV1::InvalidRgbaLength);
    }

    let mut shape_bounds: Option<BeginnerRecognitionBoundsV1> = None;
    for (index, pixel) in rgba.chunks_exact(4).enumerate() {
        if pixel[3] == 0 {
            continue;
        }
        if marker_kind(pixel).is_none() {
            return Err(BeginnerRecognitionErrorV1::UnsupportedMarker);
        }
        let x = (index % width as usize) as u32;
        let y = (index / width as usize) as u32;
        shape_bounds = Some(match shape_bounds {
            Some(bounds) => BeginnerRecognitionBoundsV1 {
                min_x: bounds.min_x.min(x),
                min_y: bounds.min_y.min(y),
                max_x: bounds.max_x.max(x),
                max_y: bounds.max_y.max(y),
            },
            None => BeginnerRecognitionBoundsV1 {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            },
        });
    }
    let shape_bounds = shape_bounds.ok_or(BeginnerRecognitionErrorV1::EmptyShape)?;

    let mut visited = vec![false; pixels];
    let mut part_counts = [0_u8; 7];
    let mut skeleton_segments = Vec::new();
    let mut component_count = 0_usize;
    for index in 0..pixels {
        if visited[index] || rgba[index * 4 + 3] == 0 {
            continue;
        }
        let marker = marker_kind(&rgba[index * 4..index * 4 + 4])
            .ok_or(BeginnerRecognitionErrorV1::UnsupportedMarker)?;
        let component =
            collect_component(index, width as usize, height as usize, rgba, &mut visited);
        component_count += 1;
        if component_count > MAX_BEGINNER_RECOGNITION_COMPONENTS_V1 {
            return Err(BeginnerRecognitionErrorV1::ComponentLimit);
        }
        match marker {
            MarkerKind::Part(part_index) => {
                part_counts[part_index] = part_counts[part_index]
                    .checked_add(1)
                    .ok_or(BeginnerRecognitionErrorV1::PartLimit)?;
            }
            MarkerKind::Skeleton => {
                if component.len() < 2 || skeleton_segments.len() >= 64 {
                    return Err(BeginnerRecognitionErrorV1::SkeletonLimit);
                }
                skeleton_segments.push(skeleton_from_component(
                    skeleton_segments.len() as u16,
                    width as usize,
                    &component,
                ));
            }
        }
    }
    let part_total = part_counts
        .iter()
        .map(|count| u16::from(*count))
        .sum::<u16>();
    if part_total > 32 {
        return Err(BeginnerRecognitionErrorV1::PartLimit);
    }
    let kinds = [
        BeginnerTargetPartKindV1::Head,
        BeginnerTargetPartKindV1::Torso,
        BeginnerTargetPartKindV1::Leg,
        BeginnerTargetPartKindV1::Horn,
        BeginnerTargetPartKindV1::Ear,
        BeginnerTargetPartKindV1::Wing,
        BeginnerTargetPartKindV1::Fin,
        BeginnerTargetPartKindV1::Antenna,
        BeginnerTargetPartKindV1::Tail,
    ];
    let target_parts = kinds
        .into_iter()
        .zip(part_counts)
        .filter_map(|(kind, count)| {
            (count > 0).then_some(BeginnerTargetPartRecordV1 { kind, count })
        })
        .collect();
    Ok(BeginnerRecognitionProposalV1 {
        schema_version: BEGINNER_RECOGNITION_SCHEMA_VERSION_V1,
        format: BeginnerRecognitionFormatV1::MarkerPngV1,
        source_underlay_id,
        source_asset_id,
        source_sha256,
        width,
        height,
        shape_bounds,
        target_parts,
        skeleton_segments,
        generic_body_outline_tenths_mm: Some(proposed_body_outline(shape_bounds)),
        generic_body_outline_mode: Some(BeginnerBodyOutlineModeV1::General),
        protrusions: Vec::new(),
        contour_confidence: None,
        skeleton_quality: None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MarkerKind {
    Part(usize),
    Skeleton,
}

fn marker_kind(pixel: &[u8]) -> Option<MarkerKind> {
    if pixel[3] == 0 {
        return None;
    }
    match [pixel[0], pixel[1], pixel[2], pixel[3]] {
        [255, 0, 0, 255] => Some(MarkerKind::Part(0)),
        [0, 255, 0, 255] => Some(MarkerKind::Part(1)),
        [0, 0, 255, 255] => Some(MarkerKind::Part(2)),
        [255, 255, 0, 255] => Some(MarkerKind::Part(3)),
        [255, 0, 255, 255] => Some(MarkerKind::Part(4)),
        [0, 255, 255, 255] => Some(MarkerKind::Part(5)),
        [255, 128, 0, 255] => Some(MarkerKind::Part(6)),
        [0, 0, 0, 255] => Some(MarkerKind::Skeleton),
        _ => None,
    }
}

fn collect_component(
    start: usize,
    width: usize,
    height: usize,
    rgba: &[u8],
    visited: &mut [bool],
) -> Vec<usize> {
    let marker = marker_kind(&rgba[start * 4..start * 4 + 4]);
    let mut queue = VecDeque::from([start]);
    let mut component = Vec::new();
    visited[start] = true;
    while let Some(index) = queue.pop_front() {
        component.push(index);
        let x = index % width;
        let y = index / width;
        for neighbor in [
            (x > 0).then(|| index - 1),
            (x + 1 < width).then(|| index + 1),
            (y > 0).then(|| index - width),
            (y + 1 < height).then(|| index + width),
        ]
        .into_iter()
        .flatten()
        {
            if !visited[neighbor] && marker_kind(&rgba[neighbor * 4..neighbor * 4 + 4]) == marker {
                visited[neighbor] = true;
                queue.push_back(neighbor);
            }
        }
    }
    component
}

fn skeleton_from_component(
    id: u16,
    width: usize,
    component: &[usize],
) -> BeginnerSkeletonSegmentV1 {
    let mut points = component
        .iter()
        .map(|index| (*index % width, *index / width))
        .collect::<Vec<_>>();
    points.sort_unstable();
    let (start_x, start_y) = points[0];
    let (end_x, end_y) = points[points.len() - 1];
    let min_x = points.iter().map(|point| point.0).min().unwrap_or(0);
    let max_x = points.iter().map(|point| point.0).max().unwrap_or(0);
    let min_y = points.iter().map(|point| point.1).min().unwrap_or(0);
    let max_y = points.iter().map(|point| point.1).max().unwrap_or(0);
    let thickness_pixels = (max_x - min_x + 1).min(max_y - min_y + 1).max(1);
    BeginnerSkeletonSegmentV1 {
        id,
        start: BeginnerSkeletonPointV1 {
            x_tenths_mm: (start_x as i32).saturating_mul(10),
            y_tenths_mm: (start_y as i32).saturating_mul(10),
        },
        end: BeginnerSkeletonPointV1 {
            x_tenths_mm: (end_x as i32).saturating_mul(10),
            y_tenths_mm: (end_y as i32).saturating_mul(10),
        },
        thickness_tenths_mm: u16::try_from(thickness_pixels.saturating_mul(10))
            .unwrap_or(u16::MAX)
            .min(10_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_exact_markers_in_deterministic_order() {
        let underlay = UnderlayId::new();
        let asset = AssetId::new();
        let rgba = [255, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 255, 0, 255];
        let proposal = analyze_marker_png_rgba_v1(underlay, asset, [7; 32], 4, 1, &rgba).unwrap();
        assert_eq!(proposal.target_parts.len(), 2);
        assert_eq!(
            proposal.target_parts[0].kind,
            BeginnerTargetPartKindV1::Head
        );
        assert_eq!(
            proposal.target_parts[1].kind,
            BeginnerTargetPartKindV1::Torso
        );
        assert_eq!(proposal.skeleton_segments.len(), 1);
        assert!(proposal.skeleton_quality.is_none());
        assert_eq!(proposal.source_sha256, [7; 32]);
        assert_eq!(
            proposal.generic_body_outline_mode,
            Some(BeginnerBodyOutlineModeV1::General)
        );
        assert_eq!(
            proposal
                .generic_body_outline_tenths_mm
                .as_ref()
                .map(Vec::len),
            Some(4)
        );
        assert!(proposal.protrusions.is_empty());
    }

    #[test]
    fn rejects_unknown_markers_and_resource_overflow() {
        assert_eq!(
            analyze_marker_png_rgba_v1(
                UnderlayId::new(),
                AssetId::new(),
                [0; 32],
                1,
                1,
                &[1, 2, 3, 255],
            ),
            Err(BeginnerRecognitionErrorV1::UnsupportedMarker)
        );
        assert_eq!(
            analyze_marker_png_rgba_v1(UnderlayId::new(), AssetId::new(), [0; 32], 4096, 4096, &[],),
            Err(BeginnerRecognitionErrorV1::PixelLimit)
        );
        assert_eq!(
            analyze_marker_png_rgba_v1(
                UnderlayId::new(),
                AssetId::new(),
                [0; 32],
                1,
                1,
                &[255, 0, 0],
            ),
            Err(BeginnerRecognitionErrorV1::InvalidRgbaLength)
        );

        let mut too_many_components = vec![0_u8; 129 * 4];
        for x in (0..129).step_by(2) {
            too_many_components[x * 4..x * 4 + 4].copy_from_slice(&[255, 0, 0, 255]);
        }
        assert_eq!(
            analyze_marker_png_rgba_v1(
                UnderlayId::new(),
                AssetId::new(),
                [0; 32],
                129,
                1,
                &too_many_components,
            ),
            Err(BeginnerRecognitionErrorV1::ComponentLimit)
        );
    }

    #[test]
    fn recognizes_one_bounded_silhouette_without_inferring_parts() {
        let mut rgba = vec![0_u8; 4 * 4 * 4];
        for y in 1..=2 {
            for x in 0..=2 {
                rgba[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
        let proposal =
            analyze_silhouette_png_rgba_v1(UnderlayId::new(), AssetId::new(), [9; 32], 4, 4, &rgba)
                .unwrap();
        assert_eq!(
            proposal.format,
            BeginnerRecognitionFormatV1::SilhouettePngV1
        );
        assert!(proposal.target_parts.is_empty());
        assert!((1..=MAX_BEGINNER_MEDIAL_AXIS_BARS_V1).contains(&proposal.skeleton_segments.len()));
        assert_eq!(proposal.skeleton_quality.as_ref().unwrap().bar_limit, 32);
        assert_eq!(proposal.shape_bounds.min_y, 1);
        assert_eq!(proposal.protrusions.len(), 2);
        assert!(proposal.protrusions.iter().all(|target| {
            target
                .local_outline_tenths_mm
                .as_ref()
                .is_some_and(|outline| outline.len() == 3)
        }));
        assert_eq!(
            proposal.generic_body_outline_mode,
            Some(BeginnerBodyOutlineModeV1::General)
        );
        assert_eq!(
            proposal
                .generic_body_outline_tenths_mm
                .as_ref()
                .map(Vec::len),
            Some(4)
        );
        let mut luminance_with_noise = vec![255_u8; 6 * 6 * 4];
        for y in 1..=3 {
            for x in 1..=3 {
                luminance_with_noise[(y * 6 + x) * 4..(y * 6 + x) * 4 + 4]
                    .copy_from_slice(&[32, 48, 64, 220]);
            }
        }
        luminance_with_noise[35 * 4..36 * 4].copy_from_slice(&[0, 0, 0, 255]);
        assert!(
            analyze_silhouette_png_rgba_v1(
                UnderlayId::new(),
                AssetId::new(),
                [8; 32],
                6,
                6,
                &luminance_with_noise,
            )
            .is_ok()
        );
    }

    #[test]
    fn simplifies_dense_silhouette_to_bounded_canonical_contour() {
        let mut rgba = vec![0_u8; 7 * 7 * 4];
        for (y, minimum, maximum) in [
            (0, 2, 4),
            (1, 1, 5),
            (2, 0, 6),
            (3, 0, 6),
            (4, 0, 6),
            (5, 1, 5),
            (6, 2, 4),
        ] {
            for x in minimum..=maximum {
                rgba[(y * 7 + x) * 4..(y * 7 + x) * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
        let proposal =
            analyze_silhouette_png_rgba_v1(UnderlayId::new(), AssetId::new(), [3; 32], 7, 7, &rgba)
                .unwrap();
        let outline = proposal.generic_body_outline_tenths_mm.unwrap();
        assert!((4..=MAX_BEGINNER_SILHOUETTE_CONTOUR_POINTS_V1).contains(&outline.len()));
        assert_eq!(outline.first(), outline.iter().min());
        assert_eq!(
            proposal.generic_body_outline_mode,
            Some(BeginnerBodyOutlineModeV1::General)
        );
    }

    #[test]
    fn silhouette_thresholds_are_inclusive_and_component_work_is_bounded() {
        let mut rgba = vec![255_u8; 4 * 4 * 4];
        for y in 1..=2 {
            for x in 0..=2 {
                rgba[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4].copy_from_slice(&[127, 127, 127, 128]);
            }
        }
        rgba[15 * 4..16 * 4].copy_from_slice(&[0, 0, 0, 127]);
        assert!(analyze_silhouette_png_rgba_v1(
            UnderlayId::new(), AssetId::new(), [4; 32], 4, 4, &rgba,
        ).is_ok());

        let mut fragmented = vec![255_u8; 17 * 17 * 4];
        for y in (0..17).step_by(2) {
            for x in (0..17).step_by(2) {
                fragmented[(y * 17 + x) * 4..(y * 17 + x) * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
        assert_eq!(
            analyze_silhouette_png_rgba_v1(
                UnderlayId::new(),
                AssetId::new(),
                [5; 32],
                17,
                17,
                &fragmented,
            ),
            Err(BeginnerRecognitionErrorV1::ComponentLimit),
        );
    }

    #[test]
    fn rejects_ambiguous_and_unsupported_silhouettes() {
        let mut disconnected = vec![0_u8; 5 * 5 * 4];
        for index in [0, 1, 5, 6, 18, 19, 23, 24] {
            disconnected[index * 4..index * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
        let multi = analyze_silhouette_png_rgba_v1(
            UnderlayId::new(),
            AssetId::new(),
            [0; 32],
            5,
            5,
            &disconnected,
        )
        .unwrap();
        assert!(multi.skeleton_segments.len() <= MAX_BEGINNER_MULTI_SILHOUETTE_BARS_V1);
        assert!(multi.contour_confidence.unwrap().explicit_override_required);
        assert!(
            multi
                .skeleton_quality
                .unwrap()
                .reasons
                .iter()
                .any(|reason| reason == "inferred_aabb_kruskal_mst_bridges")
        );

        let mut nine = vec![255_u8; 14 * 14 * 4];
        for component in 0..9 {
            let origin_x = (component % 3) * 5;
            let origin_y = (component / 3) * 5;
            for y in origin_y..origin_y + 2 {
                for x in origin_x..origin_x + 2 {
                    nine[(y * 14 + x) * 4..(y * 14 + x) * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
                }
            }
        }
        assert_eq!(
            analyze_silhouette_png_rgba_v1(
                UnderlayId::new(),
                AssetId::new(),
                [8; 32],
                14,
                14,
                &nine,
            ),
            Err(BeginnerRecognitionErrorV1::ComponentLimit)
        );
        let mut eight = vec![255_u8; 39 * 3 * 4];
        for component in 0..8 {
            for y in 0..2 {
                for x in component * 5..component * 5 + 2 {
                    eight[(y * 39 + x) * 4..(y * 39 + x) * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
                }
            }
        }
        let maximum = analyze_silhouette_png_rgba_v1(
            UnderlayId::new(),
            AssetId::new(),
            [6; 32],
            39,
            3,
            &eight,
        )
        .unwrap();
        assert_eq!(maximum.skeleton_segments.len(), 15);
        assert_eq!(
            analyze_silhouette_png_rgba_v1(
                UnderlayId::new(),
                AssetId::new(),
                [0; 32],
                2,
                2,
                &[255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            ),
            Err(BeginnerRecognitionErrorV1::AmbiguousSilhouette)
        );
        let mut holed = vec![255_u8; 5 * 5 * 4];
        for y in 0..5 {
            for x in 0..5 {
                if x == 0 || x == 4 || y == 0 || y == 4 {
                    holed[(y * 5 + x) * 4..(y * 5 + x) * 4 + 4].copy_from_slice(&[24, 24, 24, 255]);
                }
            }
        }
        assert_eq!(
            analyze_silhouette_png_rgba_v1(
                UnderlayId::new(),
                AssetId::new(),
                [0; 32],
                5,
                5,
                &holed,
            ),
            Err(BeginnerRecognitionErrorV1::AmbiguousSilhouette)
        );
    }

    #[test]
    fn outline_candidates_are_bounded_and_sorted_deterministically() {
        let mut rgba = vec![255_u8; 8 * 4 * 4];
        for index in [0, 1, 8, 9, 5, 6, 7, 13, 14, 15] {
            rgba[index * 4..index * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
        let candidates = analyze_outline_candidates_rgba_v1(8, 4, &rgba).unwrap();
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].area_pixels, 6);
        assert_eq!(candidates[0].bounds.min_x, 5);
        assert_eq!(candidates[1].area_pixels, 4);
    }
}
