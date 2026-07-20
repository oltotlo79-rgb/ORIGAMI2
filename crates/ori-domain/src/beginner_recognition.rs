use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::{
    AssetId, BeginnerSkeletonPointV1, BeginnerSkeletonSegmentV1, BeginnerTargetPartKindV1,
    BeginnerTargetPartRecordV1, UnderlayId,
};

pub const BEGINNER_RECOGNITION_SCHEMA_VERSION_V1: u32 = 1;
pub const MAX_BEGINNER_RECOGNITION_DIMENSION_V1: u32 = 4_096;
pub const MAX_BEGINNER_RECOGNITION_PIXELS_V1: usize = 4_000_000;
pub const MAX_BEGINNER_RECOGNITION_COMPONENTS_V1: usize = 64;
pub const MAX_BEGINNER_OUTLINE_CANDIDATES_V1: usize = 16;

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
        .map(|(id, (area_pixels, min_x, min_y, max_x, max_y))| BeginnerOutlineCandidateV1 {
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
        })
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
        match [pixel[0], pixel[1], pixel[2], pixel[3]] {
            [0, 0, 0, 255] => foreground[index] = true,
            [_, _, _, 0] => {}
            _ => return Err(BeginnerRecognitionErrorV1::UnsupportedSilhouette),
        }
    }
    let foreground_count = foreground.iter().filter(|value| **value).count();
    if foreground_count < 4 || foreground_count == pixels {
        return Err(BeginnerRecognitionErrorV1::AmbiguousSilhouette);
    }
    let mut visited = vec![false; pixels];
    let mut component = Vec::new();
    for start in 0..pixels {
        if !foreground[start] || visited[start] {
            continue;
        }
        if !component.is_empty() {
            return Err(BeginnerRecognitionErrorV1::AmbiguousSilhouette);
        }
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
    }
    let min_x = component
        .iter()
        .map(|index| index % width as usize)
        .min()
        .unwrap() as u32;
    let max_x = component
        .iter()
        .map(|index| index % width as usize)
        .max()
        .unwrap() as u32;
    let min_y = component
        .iter()
        .map(|index| index / width as usize)
        .min()
        .unwrap() as u32;
    let max_y = component
        .iter()
        .map(|index| index / width as usize)
        .max()
        .unwrap() as u32;
    if min_x == max_x || min_y == max_y {
        return Err(BeginnerRecognitionErrorV1::AmbiguousSilhouette);
    }
    let center_x = ((min_x + max_x) / 2) as i32 * 10;
    let center_y = ((min_y + max_y) / 2) as i32 * 10;
    let horizontal = max_x - min_x >= max_y - min_y;
    let skeleton_segments = vec![BeginnerSkeletonSegmentV1 {
        id: 0,
        start: BeginnerSkeletonPointV1 {
            x_tenths_mm: if horizontal {
                min_x as i32 * 10
            } else {
                center_x
            },
            y_tenths_mm: if horizontal {
                center_y
            } else {
                min_y as i32 * 10
            },
        },
        end: BeginnerSkeletonPointV1 {
            x_tenths_mm: if horizontal {
                max_x as i32 * 10
            } else {
                center_x
            },
            y_tenths_mm: if horizontal {
                center_y
            } else {
                max_y as i32 * 10
            },
        },
        thickness_tenths_mm: ((if horizontal {
            max_y - min_y + 1
        } else {
            max_x - min_x + 1
        }) * 10)
            .min(10_000) as u16,
    }];
    Ok(BeginnerRecognitionProposalV1 {
        schema_version: BEGINNER_RECOGNITION_SCHEMA_VERSION_V1,
        format: BeginnerRecognitionFormatV1::SilhouettePngV1,
        source_underlay_id,
        source_asset_id,
        source_sha256,
        width,
        height,
        shape_bounds: BeginnerRecognitionBoundsV1 {
            min_x,
            min_y,
            max_x,
            max_y,
        },
        target_parts: Vec::new(),
        skeleton_segments,
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
        assert_eq!(proposal.source_sha256, [7; 32]);
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
        assert_eq!(proposal.skeleton_segments.len(), 1);
        assert_eq!(proposal.shape_bounds.min_y, 1);
    }

    #[test]
    fn rejects_ambiguous_and_unsupported_silhouettes() {
        let mut disconnected = vec![0_u8; 5 * 5 * 4];
        for index in [0, 1, 5, 6, 18, 19, 23, 24] {
            disconnected[index * 4..index * 4 + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
        assert_eq!(
            analyze_silhouette_png_rgba_v1(
                UnderlayId::new(),
                AssetId::new(),
                [0; 32],
                5,
                5,
                &disconnected,
            ),
            Err(BeginnerRecognitionErrorV1::AmbiguousSilhouette)
        );
        assert_eq!(
            analyze_silhouette_png_rgba_v1(
                UnderlayId::new(),
                AssetId::new(),
                [0; 32],
                2,
                2,
                &[255, 0, 0, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            ),
            Err(BeginnerRecognitionErrorV1::UnsupportedSilhouette)
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
