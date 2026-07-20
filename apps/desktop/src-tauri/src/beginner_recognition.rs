use std::io::Cursor;

use ori_domain::{
    AssetId, BeginnerRecognitionProposalV1, ProjectId, UnderlayId, analyze_marker_png_rgba_v1,
    analyze_silhouette_png_rgba_v1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::{AppState, ProjectState, ensure_expected_project, lock_project};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RecognizeBeginnerTargetRequest {
    expected_project_instance_id: ProjectId,
    expected_project_id: ProjectId,
    expected_revision: u64,
    underlay_id: UnderlayId,
    asset_id: AssetId,
}

#[tauri::command]
pub(crate) fn recognize_beginner_silhouette(
    state: State<'_, AppState>,
    request: RecognizeBeginnerTargetRequest,
) -> Result<BeginnerRecognitionProposalV1, String> {
    let bytes = {
        let project = lock_project(&state)?;
        ensure_recognition_binding(&project, request)?;
        project
            .texture_assets
            .iter()
            .find(|asset| asset.id == request.asset_id)
            .map(|asset| asset.bytes.clone())
            .ok_or_else(|| "recognition_asset_unavailable".to_owned())?
    };
    let source_sha256: [u8; 32] = Sha256::digest(&bytes).into();
    let (width, height, rgba) = decode_general_image(&bytes)?;
    let proposal = analyze_silhouette_png_rgba_v1(
        request.underlay_id,
        request.asset_id,
        source_sha256,
        width,
        height,
        &rgba,
    )
    .map_err(|error| match error {
        ori_domain::BeginnerRecognitionErrorV1::AmbiguousSilhouette => {
            "recognition_ambiguous_silhouette".to_owned()
        }
        ori_domain::BeginnerRecognitionErrorV1::UnsupportedSilhouette => {
            "recognition_unsupported_silhouette".to_owned()
        }
        ori_domain::BeginnerRecognitionErrorV1::InvalidDimensions
        | ori_domain::BeginnerRecognitionErrorV1::PixelLimit
        | ori_domain::BeginnerRecognitionErrorV1::InvalidRgbaLength
        | ori_domain::BeginnerRecognitionErrorV1::ComponentLimit
        | ori_domain::BeginnerRecognitionErrorV1::PartLimit
        | ori_domain::BeginnerRecognitionErrorV1::SkeletonLimit => {
            "recognition_resource_limit".to_owned()
        }
        ori_domain::BeginnerRecognitionErrorV1::EmptyShape
        | ori_domain::BeginnerRecognitionErrorV1::UnsupportedMarker => {
            "recognition_unsupported_silhouette".to_owned()
        }
    })?;
    {
        let project = lock_project(&state)?;
        ensure_recognition_binding(&project, request)?;
        let live_bytes = project
            .texture_assets
            .iter()
            .find(|asset| asset.id == request.asset_id)
            .map(|asset| asset.bytes.as_slice())
            .ok_or_else(|| "recognition_asset_unavailable".to_owned())?;
        let live_hash: [u8; 32] = Sha256::digest(live_bytes).into();
        if live_hash != source_sha256 {
            return Err("recognition_asset_changed".to_owned());
        }
    }
    Ok(proposal)
}

#[derive(Debug, Serialize)]
pub(crate) struct BeginnerOutlineCandidatesResponse {
    project_instance_id: ProjectId,
    project_id: ProjectId,
    revision: u64,
    underlay_id: UnderlayId,
    asset_id: AssetId,
    candidates: Vec<ori_domain::BeginnerOutlineCandidateV1>,
}

#[tauri::command]
pub(crate) fn recognize_beginner_outline_candidates(
    state: State<'_, AppState>,
    request: RecognizeBeginnerTargetRequest,
) -> Result<BeginnerOutlineCandidatesResponse, String> {
    let bytes = {
        let project = lock_project(&state)?;
        ensure_recognition_binding(&project, request)?;
        project
            .texture_assets
            .iter()
            .find(|asset| asset.id == request.asset_id)
            .map(|asset| asset.bytes.clone())
            .ok_or_else(|| "recognition_asset_unavailable".to_owned())?
    };
    let source_hash: [u8; 32] = Sha256::digest(&bytes).into();
    let (width, height, rgba) = decode_general_image(&bytes)?;
    let candidates = ori_domain::analyze_outline_candidates_rgba_v1(width, height, &rgba)
        .map_err(|_| "recognition_resource_limit".to_owned())?;
    let project = lock_project(&state)?;
    ensure_recognition_binding(&project, request)?;
    let live = project
        .texture_assets
        .iter()
        .find(|asset| asset.id == request.asset_id)
        .ok_or_else(|| "recognition_asset_unavailable".to_owned())?;
    if <[u8; 32]>::from(Sha256::digest(&live.bytes)) != source_hash {
        return Err("recognition_asset_changed".to_owned());
    }
    Ok(BeginnerOutlineCandidatesResponse {
        project_instance_id: project.instance_id,
        project_id: project.project_id,
        revision: project.editor.revision(),
        underlay_id: request.underlay_id,
        asset_id: request.asset_id,
        candidates,
    })
}

#[tauri::command]
pub(crate) fn recognize_beginner_target(
    state: State<'_, AppState>,
    request: RecognizeBeginnerTargetRequest,
) -> Result<BeginnerRecognitionProposalV1, String> {
    let bytes = {
        let project = lock_project(&state)?;
        ensure_recognition_binding(&project, request)?;
        project
            .texture_assets
            .iter()
            .find(|asset| asset.id == request.asset_id)
            .map(|asset| asset.bytes.clone())
            .ok_or_else(|| "target recognition asset is unavailable".to_owned())?
    };
    let source_sha256: [u8; 32] = Sha256::digest(&bytes).into();
    let (width, height, rgba) = decode_marker_png(&bytes)?;
    let proposal = analyze_marker_png_rgba_v1(
        request.underlay_id,
        request.asset_id,
        source_sha256,
        width,
        height,
        &rgba,
    )
    .map_err(|error| format!("marker PNG recognition failed: {error:?}"))?;
    {
        let project = lock_project(&state)?;
        ensure_recognition_binding(&project, request)?;
        let live_bytes = project
            .texture_assets
            .iter()
            .find(|asset| asset.id == request.asset_id)
            .map(|asset| asset.bytes.as_slice())
            .ok_or_else(|| "target recognition asset is unavailable".to_owned())?;
        let live_hash: [u8; 32] = Sha256::digest(live_bytes).into();
        if live_hash != source_sha256 {
            return Err("target recognition asset changed during analysis".to_owned());
        }
    }
    Ok(proposal)
}

fn ensure_recognition_binding(
    project: &ProjectState,
    request: RecognizeBeginnerTargetRequest,
) -> Result<(), String> {
    ensure_expected_project(
        project,
        request.expected_project_instance_id,
        request.expected_project_id,
        request.expected_revision,
    )?;
    project
        .editor
        .underlays()
        .underlays
        .iter()
        .any(|underlay| underlay.id == request.underlay_id && underlay.asset == request.asset_id)
        .then_some(())
        .ok_or_else(|| "target recognition underlay binding changed".to_owned())
}

fn decode_marker_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|_| "target recognition requires a valid CRC-checked PNG".to_owned())?;
    let info = reader.info();
    let pixels = usize::try_from(info.width)
        .ok()
        .and_then(|width| {
            usize::try_from(info.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| "target recognition dimensions overflow".to_owned())?;
    if info.width > ori_domain::MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || info.height > ori_domain::MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || pixels > ori_domain::MAX_BEGINNER_RECOGNITION_PIXELS_V1
    {
        return Err("target recognition image exceeds the supported dimensions".to_owned());
    }
    let output_size = reader
        .output_buffer_size()
        .ok_or_else(|| "target recognition output size is unavailable".to_owned())?;
    if output_size != pixels.saturating_mul(4) {
        return Err("marker_png_v1 requires 8-bit RGBA pixels".to_owned());
    }
    let mut output = vec![0_u8; output_size];
    let frame = reader
        .next_frame(&mut output)
        .map_err(|_| "target recognition PNG decode failed".to_owned())?;
    if frame.color_type != png::ColorType::Rgba || frame.bit_depth != png::BitDepth::Eight {
        return Err("marker_png_v1 requires 8-bit RGBA pixels".to_owned());
    }
    output.truncate(frame.buffer_size());
    Ok((frame.width, frame.height, output))
}

fn decode_general_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|_| "recognition_requires_valid_png".to_owned())?;
    let info = reader.info();
    let pixels = usize::try_from(info.width)
        .ok()
        .and_then(|width| usize::try_from(info.height).ok()?.checked_mul(width))
        .ok_or_else(|| "recognition_resource_limit".to_owned())?;
    if info.width > ori_domain::MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || info.height > ori_domain::MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || pixels > ori_domain::MAX_BEGINNER_RECOGNITION_PIXELS_V1
    {
        return Err("recognition_resource_limit".to_owned());
    }
    let mut decoded = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| "recognition_resource_limit".to_owned())?
    ];
    let frame = reader
        .next_frame(&mut decoded)
        .map_err(|_| "recognition_requires_valid_png".to_owned())?;
    decoded.truncate(frame.buffer_size());
    let mut rgba = Vec::with_capacity(
        pixels
            .checked_mul(4)
            .ok_or_else(|| "recognition_resource_limit".to_owned())?,
    );
    match frame.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(&decoded),
        png::ColorType::Rgb => decoded.chunks_exact(3).for_each(|pixel| {
            rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
        }),
        png::ColorType::Grayscale => decoded
            .iter()
            .for_each(|value| rgba.extend_from_slice(&[*value, *value, *value, 255])),
        png::ColorType::GrayscaleAlpha => decoded.chunks_exact(2).for_each(|pixel| {
            rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
        }),
        png::ColorType::Indexed => return Err("recognition_requires_valid_png".to_owned()),
    }
    if rgba.len() != pixels * 4 {
        return Err("recognition_resource_limit".to_owned());
    }
    Ok((frame.width, frame.height, rgba))
}

fn decode_general_image(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        decode_general_png(bytes)
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        decode_general_jpeg(bytes)
    } else {
        Err("recognition_requires_png_or_jpeg".to_owned())
    }
}

fn decode_general_jpeg(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    decoder
        .read_info()
        .map_err(|_| "recognition_requires_valid_jpeg".to_owned())?;
    let info = decoder
        .info()
        .ok_or_else(|| "recognition_requires_valid_jpeg".to_owned())?;
    let width = u32::from(info.width);
    let height = u32::from(info.height);
    let pixels = usize::from(info.width)
        .checked_mul(usize::from(info.height))
        .ok_or_else(|| "recognition_resource_limit".to_owned())?;
    if width > ori_domain::MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || height > ori_domain::MAX_BEGINNER_RECOGNITION_DIMENSION_V1
        || pixels > ori_domain::MAX_BEGINNER_RECOGNITION_PIXELS_V1
    {
        return Err("recognition_resource_limit".to_owned());
    }
    let decoded = decoder
        .decode()
        .map_err(|_| "recognition_requires_valid_jpeg".to_owned())?;
    let mut rgba = Vec::with_capacity(
        pixels
            .checked_mul(4)
            .ok_or_else(|| "recognition_resource_limit".to_owned())?,
    );
    match info.pixel_format {
        jpeg_decoder::PixelFormat::L8 => decoded
            .iter()
            .for_each(|value| rgba.extend_from_slice(&[*value, *value, *value, 255])),
        jpeg_decoder::PixelFormat::RGB24 => decoded.chunks_exact(3).for_each(|pixel| {
            rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
        }),
        _ => return Err("recognition_unsupported_jpeg_color".to_owned()),
    }
    if rgba.len() != pixels * 4 {
        return Err("recognition_resource_limit".to_owned());
    }
    Ok((width, height, rgba))
}

#[cfg(test)]
mod tests {
    use super::{decode_general_image, decode_general_png, decode_marker_png};

    fn encode(color: png::ColorType, pixels: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
            encoder.set_color(color);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("header");
            writer.write_image_data(pixels).expect("pixels");
        }
        bytes
    }

    #[test]
    fn decodes_exact_rgba_png() {
        let pixels = [255, 0, 0, 255, 0, 0, 0, 0];
        let decoded = decode_marker_png(&encode(png::ColorType::Rgba, &pixels)).expect("decode");
        assert_eq!(decoded, (2, 1, pixels.to_vec()));
    }

    #[test]
    fn rejects_non_rgba_and_corrupt_png() {
        let rgb = encode(png::ColorType::Rgb, &[255, 0, 0, 0, 0, 0]);
        assert!(decode_marker_png(&rgb).is_err());

        let mut corrupt = encode(png::ColorType::Rgba, &[255, 0, 0, 255, 0, 0, 0, 0]);
        let index = corrupt.len() / 2;
        corrupt[index] ^= 0xff;
        assert!(decode_marker_png(&corrupt).is_err());
    }

    #[test]
    fn general_silhouette_decoder_deterministically_expands_rgb_and_grayscale() {
        assert_eq!(
            decode_general_png(&encode(png::ColorType::Rgb, &[10, 20, 30, 40, 50, 60]))
                .expect("RGB"),
            (2, 1, vec![10, 20, 30, 255, 40, 50, 60, 255])
        );
        assert_eq!(
            decode_general_png(&encode(png::ColorType::Grayscale, &[12, 34])).expect("grayscale"),
            (2, 1, vec![12, 12, 12, 255, 34, 34, 34, 255])
        );

        let mut palette_png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut palette_png, 2, 1);
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_palette(vec![10, 20, 30, 40, 50, 60]);
            let mut writer = encoder.write_header().expect("palette header");
            writer.write_image_data(&[0, 1]).expect("palette pixels");
        }
        assert_eq!(
            decode_general_png(&palette_png).expect("palette"),
            (2, 1, vec![10, 20, 30, 255, 40, 50, 60, 255])
        );
    }

    #[test]
    fn general_decoder_rejects_unknown_and_corrupt_jpeg_envelopes() {
        assert!(decode_general_image(b"not an image").is_err());
        assert!(decode_general_image(&[0xff, 0xd8, 0xff, 0xd9]).is_err());
    }
}
