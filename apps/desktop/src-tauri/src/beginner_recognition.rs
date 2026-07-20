use std::io::Cursor;

use ori_domain::{
    AssetId, BeginnerRecognitionProposalV1, ProjectId, UnderlayId, analyze_marker_png_rgba_v1,
};
use serde::Deserialize;
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

#[cfg(test)]
mod tests {
    use super::decode_marker_png;

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
}
