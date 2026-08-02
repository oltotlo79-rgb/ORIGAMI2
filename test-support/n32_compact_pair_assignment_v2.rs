//! Shared, checked compact assignment fixture for the canonical N=32 graph.

pub const N32_COMPACT_VARIABLE_COUNT_V2: usize = 32_896;
pub const N32_COMPACT_ASSIGNMENT_BYTES_V2: usize = 4_112;
pub const N32_PAIR_REGISTRY_SHA256_HEX_V2: &str =
    "328a3d9ddaa99538cf5216513f02395c72f1b760a24dd10c263eebaabf4f06b2";
pub const N32_DIRECTION_ASSIGNMENT_SHA256_HEX_V2: &str =
    "4a9f5939b2c3131497385051c65a6f0afba79c090e8c21809ab0f88afc7f0fc3";
pub const N32_COMPACT_BITS_HEX_V2: &str = include_str!("n32_compact_pair_assignment_v2.hex");

pub fn decode_lower_hex_v2(encoded: &str) -> Vec<u8> {
    let encoded = encoded.trim();
    assert!(encoded.len().is_multiple_of(2));
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let nibble = |value: u8| match value {
                b'0'..=b'9' => value - b'0',
                b'a'..=b'f' => value - b'a' + 10,
                _ => panic!("compact fixture must be lowercase hexadecimal"),
            };
            (nibble(pair[0]) << 4) | nibble(pair[1])
        })
        .collect()
}

pub fn n32_compact_pair_assignment_v2() -> (usize, [u8; 32], Vec<u8>) {
    let direction_bits = decode_lower_hex_v2(N32_COMPACT_BITS_HEX_V2);
    let registry_digest = decode_lower_hex_v2(N32_PAIR_REGISTRY_SHA256_HEX_V2)
        .try_into()
        .expect("N=32 registry digest is exactly 32 bytes");
    assert_eq!(direction_bits.len(), N32_COMPACT_ASSIGNMENT_BYTES_V2);
    (
        N32_COMPACT_VARIABLE_COUNT_V2,
        registry_digest,
        direction_bits,
    )
}
