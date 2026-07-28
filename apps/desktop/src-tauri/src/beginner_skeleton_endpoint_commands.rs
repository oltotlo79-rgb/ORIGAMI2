//! Deterministic native boundary for beginner skeleton endpoint construction.
//!
//! The WebView supplies only the user's original scalar inputs. It never
//! computes or authorizes the persisted endpoint. This command is read-only;
//! the caller may place the returned integer points into an editable draft.

use serde::{Deserialize, Serialize};

const BEGINNER_SKELETON_ENDPOINT_SCHEMA_VERSION_V1: u32 = 1;
const BEGINNER_SKELETON_ENDPOINT_MODEL_ID_V1: &str =
    "ori_beginner_skeleton_endpoint_binary64_ecmascript_round_tenths_v1";
const BEGINNER_SKELETON_ENDPOINT_INVALID_MESSAGE: &str =
    "beginner_skeleton_endpoint_request_invalid";
const MAX_BEGINNER_SKELETON_COORDINATE_MM: f64 = 10_000.0;
const MIN_BEGINNER_SKELETON_LENGTH_MM: f64 = 0.1;
const MAX_BEGINNER_SKELETON_LENGTH_MM: f64 = 10_000.0;
const MAX_BEGINNER_SKELETON_ANGLE_DEGREES: f64 = 360.0;
const MAX_BEGINNER_SKELETON_COORDINATE_TENTHS_MM: i32 = 100_000;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct BeginnerSkeletonEndpointRequestV1 {
    schema_version: u32,
    endpoint_model_id: String,
    transcendental_model_id: String,
    start_x_mm: f64,
    start_y_mm: f64,
    length_mm: f64,
    angle_degrees: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(super) struct BeginnerSkeletonEndpointResponseV1 {
    schema_version: u32,
    endpoint_model_id: &'static str,
    transcendental_model_id: &'static str,
    request_start_x_mm: f64,
    request_start_y_mm: f64,
    request_length_mm: f64,
    request_angle_degrees: f64,
    endpoint_x_mm: f64,
    endpoint_y_mm: f64,
    endpoint_x_bits_hex: String,
    endpoint_y_bits_hex: String,
    start_tenths_mm: [i32; 2],
    end_tenths_mm: [i32; 2],
    authorizes_project_mutation: bool,
}

#[tauri::command]
pub(super) fn resolve_beginner_skeleton_endpoint_v1(
    request: BeginnerSkeletonEndpointRequestV1,
) -> Result<BeginnerSkeletonEndpointResponseV1, String> {
    resolve_beginner_skeleton_endpoint_inner_v1(request)
        .ok_or_else(|| BEGINNER_SKELETON_ENDPOINT_INVALID_MESSAGE.to_owned())
}

fn resolve_beginner_skeleton_endpoint_inner_v1(
    request: BeginnerSkeletonEndpointRequestV1,
) -> Option<BeginnerSkeletonEndpointResponseV1> {
    if request.schema_version != BEGINNER_SKELETON_ENDPOINT_SCHEMA_VERSION_V1
        || request.endpoint_model_id != BEGINNER_SKELETON_ENDPOINT_MODEL_ID_V1
        || request.transcendental_model_id != ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
        || ![
            request.start_x_mm,
            request.start_y_mm,
            request.length_mm,
            request.angle_degrees,
        ]
        .into_iter()
        .all(f64::is_finite)
        || request.start_x_mm.abs() > MAX_BEGINNER_SKELETON_COORDINATE_MM
        || request.start_y_mm.abs() > MAX_BEGINNER_SKELETON_COORDINATE_MM
        || !(MIN_BEGINNER_SKELETON_LENGTH_MM..=MAX_BEGINNER_SKELETON_LENGTH_MM)
            .contains(&request.length_mm)
        || request.angle_degrees.abs() > MAX_BEGINNER_SKELETON_ANGLE_DEGREES
    {
        return None;
    }

    let start_x_mm = canonical_zero(request.start_x_mm);
    let start_y_mm = canonical_zero(request.start_y_mm);
    let length_mm = canonical_zero(request.length_mm);
    let angle_degrees = canonical_zero(request.angle_degrees);
    let (endpoint_x_mm, endpoint_y_mm) = ori_numeric::deterministic_polar_endpoint_v2(
        start_x_mm,
        start_y_mm,
        length_mm,
        angle_degrees,
    )
    .ok()?;
    let start_tenths_mm = [
        ecmascript_round_tenths_mm_v1(start_x_mm)?,
        ecmascript_round_tenths_mm_v1(start_y_mm)?,
    ];
    let end_tenths_mm = [
        ecmascript_round_tenths_mm_v1(endpoint_x_mm)?,
        ecmascript_round_tenths_mm_v1(endpoint_y_mm)?,
    ];
    if start_tenths_mm == end_tenths_mm {
        return None;
    }

    Some(BeginnerSkeletonEndpointResponseV1 {
        schema_version: BEGINNER_SKELETON_ENDPOINT_SCHEMA_VERSION_V1,
        endpoint_model_id: BEGINNER_SKELETON_ENDPOINT_MODEL_ID_V1,
        transcendental_model_id: ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
        request_start_x_mm: start_x_mm,
        request_start_y_mm: start_y_mm,
        request_length_mm: length_mm,
        request_angle_degrees: angle_degrees,
        endpoint_x_mm,
        endpoint_y_mm,
        endpoint_x_bits_hex: format!("{:016x}", endpoint_x_mm.to_bits()),
        endpoint_y_bits_hex: format!("{:016x}", endpoint_y_mm.to_bits()),
        start_tenths_mm,
        end_tenths_mm,
        authorizes_project_mutation: false,
    })
}

fn ecmascript_round_tenths_mm_v1(value_mm: f64) -> Option<i32> {
    let scaled = value_mm * 10.0;
    if !scaled.is_finite() {
        return None;
    }
    ecmascript_round_binary64_to_tenths_v1(scaled)
}

fn ecmascript_round_binary64_to_tenths_v1(scaled: f64) -> Option<i32> {
    if !scaled.is_finite() {
        return None;
    }
    // Adding 0.5 first can itself round upward below the half boundary. Split
    // the integral and fractional parts so the comparison remains bit-exact.
    // ECMAScript Math.round selects +infinity at an exact half.
    let floor = scaled.floor();
    let fraction = scaled - floor;
    let rounded = if fraction < 0.5 { floor } else { floor + 1.0 };
    if !rounded.is_finite()
        || rounded < f64::from(-MAX_BEGINNER_SKELETON_COORDINATE_TENTHS_MM)
        || rounded > f64::from(MAX_BEGINNER_SKELETON_COORDINATE_TENTHS_MM)
    {
        return None;
    }
    Some(rounded as i32)
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(angle_degrees: f64) -> BeginnerSkeletonEndpointRequestV1 {
        BeginnerSkeletonEndpointRequestV1 {
            schema_version: BEGINNER_SKELETON_ENDPOINT_SCHEMA_VERSION_V1,
            endpoint_model_id: BEGINNER_SKELETON_ENDPOINT_MODEL_ID_V1.to_owned(),
            transcendental_model_id: ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1
                .to_owned(),
            start_x_mm: 1.0,
            start_y_mm: 2.0,
            length_mm: 3.0,
            angle_degrees,
        }
    }

    #[test]
    fn cardinal_and_adjacent_angles_use_the_frozen_endpoint_bits() {
        for angle_degrees in [
            f64::from_bits(90.0_f64.to_bits() - 1),
            90.0,
            f64::from_bits(90.0_f64.to_bits() + 1),
        ] {
            let response = resolve_beginner_skeleton_endpoint_inner_v1(request(angle_degrees))
                .expect("valid endpoint");
            let expected =
                ori_numeric::deterministic_polar_endpoint_v2(1.0, 2.0, 3.0, angle_degrees)
                    .expect("deterministic endpoint");
            assert_eq!(response.endpoint_x_mm.to_bits(), expected.0.to_bits());
            assert_eq!(response.endpoint_y_mm.to_bits(), expected.1.to_bits());
            assert_eq!(
                response.endpoint_x_bits_hex,
                format!("{:016x}", expected.0.to_bits())
            );
            assert_eq!(
                response.endpoint_y_bits_hex,
                format!("{:016x}", expected.1.to_bits())
            );
            assert!(!response.authorizes_project_mutation);
        }
    }

    #[test]
    fn preserves_ecmascript_negative_half_rounding() {
        assert_eq!(ecmascript_round_tenths_mm_v1(-0.15), Some(-1));
        assert_eq!(ecmascript_round_tenths_mm_v1(0.15), Some(2));
        assert_eq!(ecmascript_round_tenths_mm_v1(-0.05), Some(0));
    }

    #[test]
    fn rounds_adjacent_half_bits_without_preaddition_carry() {
        let positive_half = 0.5_f64;
        assert_eq!(
            ecmascript_round_binary64_to_tenths_v1(positive_half.next_down()),
            Some(0)
        );
        assert_eq!(
            ecmascript_round_binary64_to_tenths_v1(positive_half),
            Some(1)
        );
        assert_eq!(
            ecmascript_round_binary64_to_tenths_v1(positive_half.next_up()),
            Some(1)
        );
        let negative_half = -0.5_f64;
        assert_eq!(
            ecmascript_round_binary64_to_tenths_v1(negative_half.next_down()),
            Some(-1)
        );
        assert_eq!(
            ecmascript_round_binary64_to_tenths_v1(negative_half),
            Some(0)
        );
        assert_eq!(
            ecmascript_round_binary64_to_tenths_v1(negative_half.next_up()),
            Some(0)
        );
        assert_eq!(
            ecmascript_round_binary64_to_tenths_v1(f64::from_bits(0x3fdf_ffff_ffff_ffff)),
            Some(0)
        );
        assert_eq!(
            ecmascript_round_tenths_mm_v1(0.049_999_999_999_999_996),
            Some(0)
        );
    }

    #[test]
    fn invalid_models_bounds_nonfinite_and_collapsed_segments_fail_closed() {
        let mut invalid_schema = request(0.0);
        invalid_schema.schema_version = 2;
        assert!(resolve_beginner_skeleton_endpoint_inner_v1(invalid_schema).is_none());

        let mut invalid_model = request(0.0);
        invalid_model.endpoint_model_id = "forged".to_owned();
        assert!(resolve_beginner_skeleton_endpoint_inner_v1(invalid_model).is_none());

        let mut invalid_transcendental = request(0.0);
        invalid_transcendental.transcendental_model_id = "forged".to_owned();
        assert!(resolve_beginner_skeleton_endpoint_inner_v1(invalid_transcendental).is_none());

        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut invalid = request(0.0);
            invalid.angle_degrees = value;
            assert!(resolve_beginner_skeleton_endpoint_inner_v1(invalid).is_none());
        }

        let mut out_of_bounds = request(0.0);
        out_of_bounds.start_x_mm = 10_000.0;
        out_of_bounds.length_mm = 10_000.0;
        assert!(resolve_beginner_skeleton_endpoint_inner_v1(out_of_bounds).is_none());

        let mut collapsed = request(0.0);
        collapsed.length_mm = 0.1;
        collapsed.start_x_mm = -0.04;
        collapsed.start_y_mm = -0.04;
        collapsed.angle_degrees = 45.0;
        assert!(resolve_beginner_skeleton_endpoint_inner_v1(collapsed).is_none());
    }

    #[test]
    fn request_dto_rejects_unknown_and_missing_model_fields() {
        let valid = serde_json::json!({
            "schemaVersion": 1,
            "endpointModelId": BEGINNER_SKELETON_ENDPOINT_MODEL_ID_V1,
            "transcendentalModelId":
                ori_numeric::DETERMINISTIC_TRANSCENDENTAL_MODEL_ID_V1,
            "startXMm": 0.0,
            "startYMm": 0.0,
            "lengthMm": 10.0,
            "angleDegrees": 90.0,
        });
        assert!(serde_json::from_value::<BeginnerSkeletonEndpointRequestV1>(valid.clone()).is_ok());
        let mut unknown = valid.clone();
        unknown
            .as_object_mut()
            .expect("request object")
            .insert("privateWitness".to_owned(), serde_json::json!(true));
        assert!(serde_json::from_value::<BeginnerSkeletonEndpointRequestV1>(unknown).is_err());
        let mut missing_model = valid;
        missing_model
            .as_object_mut()
            .expect("request object")
            .remove("endpointModelId");
        assert!(
            serde_json::from_value::<BeginnerSkeletonEndpointRequestV1>(missing_model).is_err()
        );
    }
}
