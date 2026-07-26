//! Shared scalar validation for native crease-pattern import commands.
//!
//! Format-specific staging, conversion, and wire DTOs remain in their FOLD
//! and SVG command modules.

pub(super) fn validate_import_scale(millimeters_per_unit: f64) -> Result<(), String> {
    if !millimeters_per_unit.is_finite() || millimeters_per_unit <= 0.0 {
        return Err("import scale must be a finite number greater than zero".to_owned());
    }
    if millimeters_per_unit > 1_000_000_000.0 {
        return Err("import scale must not exceed 1,000,000,000 mm per unit".to_owned());
    }
    Ok(())
}
