//! Shared, checked compact assignment fixture for the canonical N=33 graph.

pub const N33_COMPACT_VARIABLE_COUNT_V2: usize = 34_980;
pub const N33_COMPACT_ASSIGNMENT_BYTES_V2: usize = 4_373;
pub const N33_PAIR_REGISTRY_SHA256_HEX_V2: &str =
    "d6b9e522cdb878fe53fd959cb41c8042e7ab29189c059e90bbff7594d6271935";
pub const N33_COMPACT_ASSIGNMENT_SHA256_HEX_V2: &str =
    "7181d8c0c37edb6434fadd8c827dbaffb915c4f1f13082a431f5492e51237b52";
pub const N33_COMPACT_BITS_HEX_V2: &str = include_str!("n33_compact_pair_assignment_v2.hex");

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

pub fn n33_compact_pair_assignment_v2() -> (usize, [u8; 32], Vec<u8>) {
    let direction_bits = decode_lower_hex_v2(N33_COMPACT_BITS_HEX_V2);
    let registry_digest = decode_lower_hex_v2(N33_PAIR_REGISTRY_SHA256_HEX_V2)
        .try_into()
        .expect("N=33 registry digest is exactly 32 bytes");
    assert_eq!(direction_bits.len(), N33_COMPACT_ASSIGNMENT_BYTES_V2);
    (
        N33_COMPACT_VARIABLE_COUNT_V2,
        registry_digest,
        direction_bits,
    )
}

pub fn n33_compact_pair_assignment_sha256_v2() -> [u8; 32] {
    decode_lower_hex_v2(N33_COMPACT_ASSIGNMENT_SHA256_HEX_V2)
        .try_into()
        .expect("N=33 compact assignment digest is exactly 32 bytes")
}
