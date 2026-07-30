use thiserror::Error;

/// Frozen binary64 transcendental semantics used by replayable proof and
/// persistence contracts.
///
/// Changing the pinned `libm` version, the cardinal-angle branches, either
/// angle-conversion constant, or zero canonicalization requires a new model
/// identifier and must not silently change this model.
pub const DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1: &str =
    "ori_binary64_libm_0_2_16_no_arch_cardinal_v1";

// Freeze the rounded binary64 coefficient rather than asking each compiler
// version to constant-fold `PI / 180.0` as part of the replay contract.
const DEGREES_TO_RADIANS_V1: f64 = f64::from_bits(0x3f91_df46_a252_9d39);
const RADIANS_TO_DEGREES_V1: f64 = f64::from_bits(0x404c_a5dc_1a63_c1f8);

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum DeterministicTranscendentalError {
    #[error("deterministic transcendental input is not finite")]
    NonFiniteInput,
    #[error("deterministic transcendental result is not finite")]
    NonFiniteResult,
}

/// Whether this build target is covered by the v1 cross-runtime replay claim.
///
/// V1 covers the continuously tested release/CI triples with a golden-bit
/// corpus: x86-64 Windows/MSVC, x86-64 Linux/GNU, and AArch64 release macOS.
/// The frozen pure-Rust kernel also compiles on other targets, but proof DTOs
/// must remain fail-closed until their target receives equivalent release and
/// CI evidence.
#[must_use]
pub const fn deterministic_transcendental_model_supported_v1() -> bool {
    deterministic_transcendental_target_facts_supported_v1([
        cfg!(target_pointer_width = "64"),
        cfg!(target_endian = "little"),
        cfg!(target_arch = "x86_64"),
        cfg!(target_arch = "aarch64"),
        cfg!(target_os = "windows"),
        cfg!(target_os = "linux"),
        cfg!(target_os = "macos"),
        cfg!(target_env = "msvc"),
        cfg!(target_env = "gnu"),
    ])
}

const fn deterministic_transcendental_target_facts_supported_v1(
    [
        pointer_width_64,
        little_endian,
        x86_64,
        aarch64,
        windows,
        linux,
        macos,
        msvc,
        gnu,
    ]: [bool; 9],
) -> bool {
    pointer_width_64
        && little_endian
        && ((x86_64 && windows && msvc) || (x86_64 && linux && gnu) || (aarch64 && macos))
}

#[cfg(all(
    target_pointer_width = "64",
    target_endian = "little",
    any(
        all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"),
        all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"),
        all(target_arch = "aarch64", target_os = "macos")
    )
))]
const _: () = assert!(deterministic_transcendental_model_supported_v1());

#[cfg(not(all(
    target_pointer_width = "64",
    target_endian = "little",
    any(
        all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"),
        all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"),
        all(target_arch = "aarch64", target_os = "macos")
    )
)))]
const _: () = assert!(!deterministic_transcendental_model_supported_v1());

pub fn deterministic_sin_v1(radians: f64) -> Result<f64, DeterministicTranscendentalError> {
    finite_unary(radians, libm::sin)
}

pub fn deterministic_cos_v1(radians: f64) -> Result<f64, DeterministicTranscendentalError> {
    finite_unary(radians, libm::cos)
}

pub fn deterministic_sin_cos_v1(
    radians: f64,
) -> Result<(f64, f64), DeterministicTranscendentalError> {
    if !radians.is_finite() {
        return Err(DeterministicTranscendentalError::NonFiniteInput);
    }
    finite_pair(libm::sincos(canonical_zero(radians)))
}

pub fn deterministic_degrees_to_radians_v1(
    angle_degrees: f64,
) -> Result<f64, DeterministicTranscendentalError> {
    if !angle_degrees.is_finite() {
        return Err(DeterministicTranscendentalError::NonFiniteInput);
    }
    finite_result(canonical_zero(angle_degrees) * DEGREES_TO_RADIANS_V1)
}

pub fn deterministic_radians_to_degrees_v1(
    angle_radians: f64,
) -> Result<f64, DeterministicTranscendentalError> {
    if !angle_radians.is_finite() {
        return Err(DeterministicTranscendentalError::NonFiniteInput);
    }
    finite_result(canonical_zero(angle_radians) * RADIANS_TO_DEGREES_V1)
}

pub fn deterministic_sin_cos_degrees_v1(
    angle_degrees: f64,
) -> Result<(f64, f64), DeterministicTranscendentalError> {
    if !angle_degrees.is_finite() {
        return Err(DeterministicTranscendentalError::NonFiniteInput);
    }
    let reduced = canonical_zero(canonical_zero(angle_degrees) % 360.0);
    let exact = match reduced {
        0.0 => Some((0.0, 1.0)),
        90.0 | -270.0 => Some((1.0, 0.0)),
        -90.0 | 270.0 => Some((-1.0, 0.0)),
        180.0 | -180.0 => Some((0.0, -1.0)),
        _ => None,
    };
    exact.map_or_else(
        || deterministic_degrees_to_radians_v1(reduced).and_then(deterministic_sin_cos_v1),
        Ok,
    )
}

/// Computes the schema-V2 polar endpoint in one frozen operation order.
///
/// Callers must persist and validate the returned endpoint bits directly.
/// Reordering the multiplications/additions or replacing the degree kernel
/// requires a new persistence schema and transcendental model identifier.
pub fn deterministic_polar_endpoint_v2(
    start_x: f64,
    start_y: f64,
    length: f64,
    angle_degrees: f64,
) -> Result<(f64, f64), DeterministicTranscendentalError> {
    if !start_x.is_finite() || !start_y.is_finite() || !length.is_finite() {
        return Err(DeterministicTranscendentalError::NonFiniteInput);
    }
    let (sin, cos) = deterministic_sin_cos_degrees_v1(angle_degrees)?;
    let delta_x = length * cos;
    let delta_y = length * sin;
    finite_pair((start_x + delta_x, start_y + delta_y))
}

pub fn deterministic_atan2_v1(y: f64, x: f64) -> Result<f64, DeterministicTranscendentalError> {
    if !y.is_finite() || !x.is_finite() {
        return Err(DeterministicTranscendentalError::NonFiniteInput);
    }
    // `atan2` must retain signed-zero inputs because they select a branch-cut
    // and quadrant result. Only a zero result is canonicalized afterwards.
    finite_result(libm::atan2(y, x))
}

pub fn deterministic_hypot_v1(x: f64, y: f64) -> Result<f64, DeterministicTranscendentalError> {
    finite_binary(x, y, libm::hypot)
}

fn finite_unary(
    input: f64,
    operation: impl FnOnce(f64) -> f64,
) -> Result<f64, DeterministicTranscendentalError> {
    if !input.is_finite() {
        return Err(DeterministicTranscendentalError::NonFiniteInput);
    }
    finite_result(operation(canonical_zero(input)))
}

fn finite_binary(
    left: f64,
    right: f64,
    operation: impl FnOnce(f64, f64) -> f64,
) -> Result<f64, DeterministicTranscendentalError> {
    if !left.is_finite() || !right.is_finite() {
        return Err(DeterministicTranscendentalError::NonFiniteInput);
    }
    finite_result(operation(canonical_zero(left), canonical_zero(right)))
}

fn finite_pair((left, right): (f64, f64)) -> Result<(f64, f64), DeterministicTranscendentalError> {
    if !left.is_finite() || !right.is_finite() {
        return Err(DeterministicTranscendentalError::NonFiniteResult);
    }
    Ok((canonical_zero(left), canonical_zero(right)))
}

fn finite_result(value: f64) -> Result<f64, DeterministicTranscendentalError> {
    if value.is_finite() {
        Ok(canonical_zero(value))
    } else {
        Err(DeterministicTranscendentalError::NonFiniteResult)
    }
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinal_degrees_are_exact_and_zero_is_canonical() {
        assert_eq!(
            deterministic_degrees_to_radians_v1(180.0).map(f64::to_bits),
            Ok(core::f64::consts::PI.to_bits())
        );
        assert_eq!(
            deterministic_radians_to_degrees_v1(core::f64::consts::PI).map(f64::to_bits),
            Ok(180.0_f64.to_bits())
        );
        assert_eq!(
            deterministic_radians_to_degrees_v1(-0.0).map(f64::to_bits),
            Ok(0.0_f64.to_bits())
        );
        assert_eq!(deterministic_sin_cos_degrees_v1(-0.0), Ok((0.0, 1.0)));
        assert_eq!(deterministic_sin_cos_degrees_v1(90.0), Ok((1.0, 0.0)));
        assert_eq!(deterministic_sin_cos_degrees_v1(-270.0), Ok((1.0, 0.0)));
        assert_eq!(deterministic_sin_cos_degrees_v1(270.0), Ok((-1.0, 0.0)));
        assert_eq!(deterministic_sin_cos_degrees_v1(540.0), Ok((0.0, -1.0)));
        for (y, x, expected) in [
            (0.0, 0.0, 0.0),
            (-0.0, 0.0, 0.0),
            (0.0, -0.0, core::f64::consts::PI),
            (-0.0, -0.0, -core::f64::consts::PI),
        ] {
            assert_eq!(
                deterministic_atan2_v1(y, x).map(f64::to_bits),
                Ok(expected.to_bits())
            );
        }
    }

    #[test]
    fn non_finite_inputs_and_outputs_fail_closed() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                deterministic_sin_v1(value),
                Err(DeterministicTranscendentalError::NonFiniteInput)
            );
            assert_eq!(
                deterministic_sin_cos_degrees_v1(value),
                Err(DeterministicTranscendentalError::NonFiniteInput)
            );
            assert_eq!(
                deterministic_degrees_to_radians_v1(value),
                Err(DeterministicTranscendentalError::NonFiniteInput)
            );
            assert_eq!(
                deterministic_radians_to_degrees_v1(value),
                Err(DeterministicTranscendentalError::NonFiniteInput)
            );
            assert_eq!(
                deterministic_hypot_v1(value, 1.0),
                Err(DeterministicTranscendentalError::NonFiniteInput)
            );
            for arguments in [
                (value, 0.0, 1.0, 0.0),
                (0.0, value, 1.0, 0.0),
                (0.0, 0.0, value, 0.0),
                (0.0, 0.0, 1.0, value),
            ] {
                assert_eq!(
                    deterministic_polar_endpoint_v2(
                        arguments.0,
                        arguments.1,
                        arguments.2,
                        arguments.3,
                    ),
                    Err(DeterministicTranscendentalError::NonFiniteInput)
                );
            }
        }
        assert_eq!(
            deterministic_hypot_v1(f64::MAX, f64::MAX),
            Err(DeterministicTranscendentalError::NonFiniteResult)
        );
        assert_eq!(
            deterministic_radians_to_degrees_v1(f64::MAX),
            Err(DeterministicTranscendentalError::NonFiniteResult)
        );
        assert_eq!(
            deterministic_polar_endpoint_v2(f64::MAX, 0.0, f64::MAX, 0.0),
            Err(DeterministicTranscendentalError::NonFiniteResult)
        );
    }

    #[test]
    fn v1_golden_binary64_corpus_is_bit_exact() {
        let unary = [
            (
                0x3ff0_0000_0000_0000,
                0x3fea_ed54_8f09_0cee,
                0x3fe1_4a28_0fb5_068c,
            ),
            (
                0xbff0_0000_0000_0000,
                0xbfea_ed54_8f09_0cee,
                0x3fe1_4a28_0fb5_068c,
            ),
            (
                0x3fe0_c152_382d_7366,
                0x3fe0_0000_0000_0000,
                0x3feb_b67a_e858_4caa,
            ),
            (
                0x3fe9_21fb_5444_2d18,
                0x3fe6_a09e_667f_3bcc,
                0x3fe6_a09e_667f_3bcd,
            ),
            (
                0x3ff9_21fb_5444_2d18,
                0x3ff0_0000_0000_0000,
                0x3c91_a626_3314_5c07,
            ),
            (
                0x01a5_6e1f_c2f8_f359,
                0x01a5_6e1f_c2f8_f359,
                0x3ff0_0000_0000_0000,
            ),
            (
                0x7e37_e43c_8800_759c,
                0xbfea_2c16_b010_e385,
                0xbfe2_6990_22ad_c4c1,
            ),
        ];
        for (input_bits, sine_bits, cosine_bits) in unary {
            let input = f64::from_bits(input_bits);
            assert_eq!(deterministic_sin_v1(input).map(f64::to_bits), Ok(sine_bits));
            assert_eq!(
                deterministic_cos_v1(input).map(f64::to_bits),
                Ok(cosine_bits)
            );
            assert_eq!(
                deterministic_sin_cos_v1(input)
                    .map(|(sine, cosine)| { (sine.to_bits(), cosine.to_bits()) }),
                Ok((sine_bits, cosine_bits))
            );
        }

        let atan2 = [
            (
                0x3ff0_0000_0000_0000,
                0x3ff0_0000_0000_0000,
                0x3fe9_21fb_5444_2d18,
            ),
            (
                0xbff0_0000_0000_0000,
                0x3ff0_0000_0000_0000,
                0xbfe9_21fb_5444_2d18,
            ),
            (
                0x3ff0_0000_0000_0000,
                0xbff0_0000_0000_0000,
                0x4002_d97c_7f33_21d2,
            ),
            (
                0xbff0_0000_0000_0000,
                0xbff0_0000_0000_0000,
                0xc002_d97c_7f33_21d2,
            ),
            (
                0x0010_0000_0000_0000,
                0x7fef_ffff_ffff_ffff,
                0x0000_0000_0000_0000,
            ),
            (
                0x0000_0000_0000_0000,
                0x8000_0000_0000_0000,
                0x4009_21fb_5444_2d18,
            ),
            (
                0x8000_0000_0000_0000,
                0x8000_0000_0000_0000,
                0xc009_21fb_5444_2d18,
            ),
        ];
        for (y_bits, x_bits, result_bits) in atan2 {
            assert_eq!(
                deterministic_atan2_v1(f64::from_bits(y_bits), f64::from_bits(x_bits))
                    .map(f64::to_bits),
                Ok(result_bits)
            );
        }

        let hypot = [
            (
                0x4008_0000_0000_0000,
                0x4010_0000_0000_0000,
                0x4014_0000_0000_0000,
            ),
            (
                0x0000_0000_0000_0001,
                0x0000_0000_0000_0002,
                0x0000_0000_0000_0002,
            ),
            (
                0x7fe1_ccf3_85eb_c8a0,
                0x7fe1_ccf3_85eb_c8a0,
                0x7fe9_2c80_954c_51f5,
            ),
        ];
        for (x_bits, y_bits, result_bits) in hypot {
            assert_eq!(
                deterministic_hypot_v1(f64::from_bits(x_bits), f64::from_bits(y_bits))
                    .map(f64::to_bits),
                Ok(result_bits)
            );
        }

        let degrees = [
            (
                0x4042_c000_0000_0000,
                0x3fe3_7af9_3f95_13ea,
                0x3fe9_6326_8b57_2492,
            ),
            (
                0xc042_c000_0000_0000,
                0xbfe3_7af9_3f95_13ea,
                0x3fe9_6326_8b57_2492,
            ),
            (
                0x4056_7fff_ffff_ffff,
                0x3ff0_0000_0000_0000,
                0x3cb4_6989_8cc5_1702,
            ),
            (
                0x4056_8000_0000_0001,
                0x3ff0_0000_0000_0000,
                0xbca7_2cec_e675_d1fd,
            ),
            (
                0x407c_2000_0000_0000,
                0x3ff0_0000_0000_0000,
                0x0000_0000_0000_0000,
            ),
            (
                0xc07c_2000_0000_0000,
                0xbff0_0000_0000_0000,
                0x0000_0000_0000_0000,
            ),
        ];
        for (degrees_bits, sine_bits, cosine_bits) in degrees {
            assert_eq!(
                deterministic_sin_cos_degrees_v1(f64::from_bits(degrees_bits))
                    .map(|(sine, cosine)| (sine.to_bits(), cosine.to_bits())),
                Ok((sine_bits, cosine_bits))
            );
        }

        let radians_to_degrees = [
            (0x3ff0_0000_0000_0000, 0x404c_a5dc_1a63_c1f8),
            (0x4009_21fb_5444_2d18, 0x4066_8000_0000_0000),
            (0x3ff9_21fb_5444_2d18, 0x4056_8000_0000_0000),
            (0x3fb9_9999_9999_999a, 0x4016_eb16_7b83_0194),
            (0xbff0_0000_0000_0000, 0xc04c_a5dc_1a63_c1f8),
            (0x0000_0000_0000_0001, 0x0000_0000_0000_0039),
        ];
        for (radians_bits, degrees_bits) in radians_to_degrees {
            assert_eq!(
                deterministic_radians_to_degrees_v1(f64::from_bits(radians_bits)).map(f64::to_bits),
                Ok(degrees_bits)
            );
        }

        let polar_endpoints = [
            (
                0x3ff4_0000_0000_0000,
                0xc004_0000_0000_0000,
                0x400e_0000_0000_0000,
                0x4042_c000_0000_0000,
                0x4010_e67a_1150_d924,
                0xbfcb_cb65_4643_d550,
            ),
            (
                0xc030_0000_0000_0000,
                0x4020_0000_0000_0000,
                0x4004_0000_0000_0000,
                0xc042_c000_0000_0000,
                0xc02c_0881_fa3a_6249,
                0x4019_e992_1c21_69c7,
            ),
            (
                0x4024_0000_0000_0000,
                0x4034_0000_0000_0000,
                0x4014_0000_0000_0000,
                0x4056_8000_0000_0000,
                0x4024_0000_0000_0000,
                0x4039_0000_0000_0000,
            ),
            (
                0x4024_0000_0000_0000,
                0x4034_0000_0000_0000,
                0x4014_0000_0000_0000,
                0x4066_8000_0000_0000,
                0x4014_0000_0000_0000,
                0x4034_0000_0000_0000,
            ),
            (
                0x0000_0000_0000_0000,
                0x0000_0000_0000_0000,
                0x0000_0000_0000_0001,
                0x0000_0000_0000_0000,
                0x0000_0000_0000_0001,
                0x0000_0000_0000_0000,
            ),
            (
                0x8000_0000_0000_0000,
                0x8000_0000_0000_0000,
                0x8000_0000_0000_0000,
                0x8000_0000_0000_0000,
                0x0000_0000_0000_0000,
                0x0000_0000_0000_0000,
            ),
        ];
        for (start_x, start_y, length, angle, expected_x, expected_y) in polar_endpoints {
            assert_eq!(
                deterministic_polar_endpoint_v2(
                    f64::from_bits(start_x),
                    f64::from_bits(start_y),
                    f64::from_bits(length),
                    f64::from_bits(angle),
                )
                .map(|(x, y)| (x.to_bits(), y.to_bits())),
                Ok((expected_x, expected_y))
            );
        }
    }

    #[test]
    fn v1_supported_target_matrix_is_explicit_and_closed() {
        for (name, facts) in [
            (
                "x86_64-pc-windows-msvc",
                [true, true, true, false, true, false, false, true, false],
            ),
            (
                "x86_64-unknown-linux-gnu",
                [true, true, true, false, false, true, false, false, true],
            ),
            (
                "aarch64-apple-darwin",
                [true, true, false, true, false, false, true, false, false],
            ),
        ] {
            assert!(
                deterministic_transcendental_target_facts_supported_v1(facts),
                "{name} must remain covered by v1"
            );
        }
        for (name, facts) in [
            (
                "32-bit x86 macOS",
                [false, true, true, false, false, false, true, false, false],
            ),
            (
                "big-endian AArch64 macOS",
                [true, false, false, true, false, false, true, false, false],
            ),
            (
                "other-architecture macOS",
                [true, true, false, false, false, false, true, false, false],
            ),
            (
                "x86-64 macOS without a native release evidence gate",
                [true, true, true, false, false, false, true, false, false],
            ),
            (
                "AArch64 Linux/GNU",
                [true, true, false, true, false, true, false, false, true],
            ),
            (
                "x86-64 Linux without GNU",
                [true, true, true, false, false, true, false, false, false],
            ),
            (
                "x86-64 Windows/GNU",
                [true, true, true, false, true, false, false, false, true],
            ),
            (
                "AArch64 Windows/MSVC",
                [true, true, false, true, true, false, false, true, false],
            ),
            (
                "x86-64 unsupported operating system",
                [true, true, true, false, false, false, false, false, false],
            ),
        ] {
            assert!(
                !deterministic_transcendental_target_facts_supported_v1(facts),
                "{name} must remain fail-closed"
            );
        }
    }

    #[test]
    fn v1_model_id_and_supported_targets_are_explicit() {
        assert_eq!(
            DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
            "ori_binary64_libm_0_2_16_no_arch_cardinal_v1"
        );
        if std::env::var("ORI_REQUIRE_SUPPORTED_TRANSCENDENTAL_TARGET").as_deref() == Ok("1") {
            assert!(
                deterministic_transcendental_model_supported_v1(),
                "the release/CI runner is not one of the target triples covered by model v1"
            );
        }
        #[cfg(all(
            target_pointer_width = "64",
            target_endian = "little",
            any(
                all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"),
                all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"),
                all(target_arch = "aarch64", target_os = "macos")
            )
        ))]
        assert!(deterministic_transcendental_model_supported_v1());
        #[cfg(not(all(
            target_pointer_width = "64",
            target_endian = "little",
            any(
                all(target_arch = "x86_64", target_os = "windows", target_env = "msvc"),
                all(target_arch = "x86_64", target_os = "linux", target_env = "gnu"),
                all(target_arch = "aarch64", target_os = "macos")
            )
        )))]
        assert!(!deterministic_transcendental_model_supported_v1());
    }
}
