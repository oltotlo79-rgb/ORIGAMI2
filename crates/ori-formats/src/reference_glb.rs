use serde_json::Value;

pub const MAX_REFERENCE_GLB_BYTES_V1: usize = 16 * 1024 * 1024;
pub const MAX_REFERENCE_GLB_VERTICES_V1: usize = 20_000;
pub const MAX_REFERENCE_GLB_TRIANGLES_V1: usize = 40_000;
const MAX_REFERENCE_GLB_JSON_BYTES_V1: usize = 2 * 1024 * 1024;
const JSON_CHUNK_TYPE: u32 = 0x4e4f_534a;
const BIN_CHUNK_TYPE: u32 = 0x004e_4942;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceGlbErrorV1 {
    Size,
    Container,
    Json,
    UnsupportedContent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceGlbGeometryV1 {
    pub positions: Vec<[f32; 3]>,
    pub triangle_indices: Vec<[u32; 3]>,
    pub material_color: [u8; 4],
}

/// Admits a passive GLB 2.0 subset for project-local visual reference.
pub fn validate_reference_glb_v1(bytes: &[u8]) -> Result<(), ReferenceGlbErrorV1> {
    if !(20..=MAX_REFERENCE_GLB_BYTES_V1).contains(&bytes.len()) {
        return Err(ReferenceGlbErrorV1::Size);
    }
    if bytes.get(..4) != Some(b"glTF")
        || read_u32(bytes, 4) != Some(2)
        || read_u32(bytes, 8) != u32::try_from(bytes.len()).ok()
    {
        return Err(ReferenceGlbErrorV1::Container);
    }
    let json_length = read_u32(bytes, 12).ok_or(ReferenceGlbErrorV1::Container)? as usize;
    if json_length == 0 || json_length > MAX_REFERENCE_GLB_JSON_BYTES_V1 {
        return Err(ReferenceGlbErrorV1::Size);
    }
    if read_u32(bytes, 16) != Some(JSON_CHUNK_TYPE) {
        return Err(ReferenceGlbErrorV1::Container);
    }
    let json_end = 20usize
        .checked_add(json_length)
        .ok_or(ReferenceGlbErrorV1::Container)?;
    let json_bytes = bytes
        .get(20..json_end)
        .ok_or(ReferenceGlbErrorV1::Container)?;
    let mut cursor = json_end;
    if cursor < bytes.len() {
        let bin_length = read_u32(bytes, cursor).ok_or(ReferenceGlbErrorV1::Container)? as usize;
        if read_u32(bytes, cursor + 4) != Some(BIN_CHUNK_TYPE) {
            return Err(ReferenceGlbErrorV1::Container);
        }
        cursor = cursor
            .checked_add(8)
            .and_then(|value| value.checked_add(bin_length))
            .ok_or(ReferenceGlbErrorV1::Container)?;
    }
    if cursor != bytes.len() {
        return Err(ReferenceGlbErrorV1::Container);
    }
    let json_text = std::str::from_utf8(json_bytes).map_err(|_| ReferenceGlbErrorV1::Json)?;
    let root: Value = serde_json::from_str(json_text.trim_end_matches([' ', '\0']))
        .map_err(|_| ReferenceGlbErrorV1::Json)?;
    let object = root.as_object().ok_or(ReferenceGlbErrorV1::Json)?;
    let asset = object
        .get("asset")
        .and_then(Value::as_object)
        .ok_or(ReferenceGlbErrorV1::UnsupportedContent)?;
    if asset.get("version").and_then(Value::as_str) != Some("2.0")
        || non_empty_array(object.get("extensionsUsed"))
        || non_empty_array(object.get("extensionsRequired"))
        || non_empty_array(object.get("animations"))
        || contains_forbidden_member(&root)
    {
        return Err(ReferenceGlbErrorV1::UnsupportedContent);
    }
    Ok(())
}

/// Extracts only bounded inert triangle geometry after the passive-container check.
pub fn read_reference_glb_geometry_v1(
    bytes: &[u8],
) -> Result<ReferenceGlbGeometryV1, ReferenceGlbErrorV1> {
    validate_reference_glb_v1(bytes)?;
    let gltf = gltf::Gltf::from_slice(bytes).map_err(|_| ReferenceGlbErrorV1::Json)?;
    let blob = gltf
        .blob
        .as_deref()
        .ok_or(ReferenceGlbErrorV1::UnsupportedContent)?;
    let mut positions = Vec::new();
    let mut triangle_indices = Vec::new();
    let mut material_color = [184, 192, 204, 255];
    for mesh in gltf.meshes() {
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                return Err(ReferenceGlbErrorV1::UnsupportedContent);
            }
            let reader = primitive.reader(|buffer| (buffer.index() == 0).then_some(blob));
            let local = reader
                .read_positions()
                .ok_or(ReferenceGlbErrorV1::UnsupportedContent)?
                .collect::<Vec<_>>();
            if local.is_empty()
                || positions.len().saturating_add(local.len()) > MAX_REFERENCE_GLB_VERTICES_V1
                || local
                    .iter()
                    .flatten()
                    .any(|coordinate| !coordinate.is_finite())
            {
                return Err(ReferenceGlbErrorV1::Size);
            }
            let base = u32::try_from(positions.len()).map_err(|_| ReferenceGlbErrorV1::Size)?;
            let indices = reader
                .read_indices()
                .map(|indices| indices.into_u32().collect::<Vec<_>>())
                .unwrap_or_else(|| (0..local.len() as u32).collect());
            if indices.len() % 3 != 0
                || indices.iter().any(|index| *index as usize >= local.len())
                || triangle_indices.len().saturating_add(indices.len() / 3)
                    > MAX_REFERENCE_GLB_TRIANGLES_V1
            {
                return Err(ReferenceGlbErrorV1::Size);
            }
            triangle_indices.extend(
                indices
                    .chunks_exact(3)
                    .map(|triangle| [base + triangle[0], base + triangle[1], base + triangle[2]]),
            );
            positions.extend(local);
            let factor = primitive
                .material()
                .pbr_metallic_roughness()
                .base_color_factor();
            material_color = factor.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    if positions.is_empty() || triangle_indices.is_empty() {
        return Err(ReferenceGlbErrorV1::UnsupportedContent);
    }
    Ok(ReferenceGlbGeometryV1 {
        positions,
        triangle_indices,
        material_color,
    })
}

fn contains_forbidden_member(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, child)| {
            matches!(key.as_str(), "uri" | "script" | "scripts") || contains_forbidden_member(child)
        }),
        Value::Array(values) => values.iter().any(contains_forbidden_member),
        _ => false,
    }
}

fn non_empty_array(value: Option<&Value>) -> bool {
    value.is_some_and(|value| value.as_array().is_none_or(|values| !values.is_empty()))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_passive_glb_and_rejects_active_or_external_content() {
        assert_eq!(
            validate_reference_glb_v1(&glb(
                r#"{"asset":{"version":"2.0"},"scenes":[{"nodes":[]}]}"#
            )),
            Ok(())
        );
        for json in [
            r#"{"asset":{"version":"1.0"}}"#,
            r#"{"asset":{"version":"2.0"},"buffers":[{"uri":"model.bin"}]}"#,
            r#"{"asset":{"version":"2.0"},"extensionsUsed":["KHR_draco_mesh_compression"]}"#,
            r#"{"asset":{"version":"2.0"},"animations":[{}]}"#,
            r#"{"asset":{"version":"2.0"},"extras":{"script":"alert(1)"}}"#,
        ] {
            assert_eq!(
                validate_reference_glb_v1(&glb(json)),
                Err(ReferenceGlbErrorV1::UnsupportedContent)
            );
        }
    }

    #[test]
    fn rejects_bad_header_and_trailing_data() {
        let valid = glb(r#"{"asset":{"version":"2.0"}}"#);
        for (offset, byte) in [(0, b'X'), (4, 1), (8, 0), (16, 0)] {
            let mut candidate = valid.clone();
            candidate[offset] = byte;
            assert!(validate_reference_glb_v1(&candidate).is_err());
        }
        let mut trailing = valid;
        trailing.extend_from_slice(&[0, 0, 0, 0]);
        assert!(validate_reference_glb_v1(&trailing).is_err());
    }

    #[test]
    fn extracts_only_bounded_triangle_geometry() {
        let json = r#"{"asset":{"version":"2.0"},"buffers":[{"byteLength":42}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":6}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]},{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1}]}]}"#;
        let mut binary = Vec::new();
        for coordinate in [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            binary.extend_from_slice(&coordinate.to_le_bytes());
        }
        for index in [0_u16, 1, 2] {
            binary.extend_from_slice(&index.to_le_bytes());
        }
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let geometry =
            read_reference_glb_geometry_v1(&glb_with_bin(json, &binary)).expect("triangle");
        assert_eq!(geometry.positions.len(), 3);
        assert_eq!(geometry.triangle_indices, vec![[0, 1, 2]]);
    }

    fn glb(json: &str) -> Vec<u8> {
        let mut json = json.as_bytes().to_vec();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let length = 20 + json.len();
        let mut bytes = Vec::with_capacity(length);
        bytes.extend_from_slice(b"glTF");
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&(length as u32).to_le_bytes());
        bytes.extend_from_slice(&(json.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&JSON_CHUNK_TYPE.to_le_bytes());
        bytes.extend_from_slice(&json);
        bytes
    }

    fn glb_with_bin(json: &str, binary: &[u8]) -> Vec<u8> {
        let mut bytes = glb(json);
        let total = bytes.len() + 8 + binary.len();
        bytes[8..12].copy_from_slice(&(total as u32).to_le_bytes());
        bytes.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&BIN_CHUNK_TYPE.to_le_bytes());
        bytes.extend_from_slice(binary);
        bytes
    }
}
