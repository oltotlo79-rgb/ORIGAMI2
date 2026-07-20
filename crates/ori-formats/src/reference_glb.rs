use serde_json::Value;

pub const MAX_REFERENCE_GLB_BYTES_V1: usize = 16 * 1024 * 1024;
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

    fn glb(json: &str) -> Vec<u8> {
        let mut json = json.as_bytes().to_vec();
        while json.len() % 4 != 0 {
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
}
