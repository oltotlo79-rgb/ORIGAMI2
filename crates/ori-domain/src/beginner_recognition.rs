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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginnerRecognitionFormatV1 {
    MarkerPngV1,
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
}
