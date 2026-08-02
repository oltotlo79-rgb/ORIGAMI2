//! Private construction kernels for dynamic general-N positive thickness.
//!
//! Nothing in this module is a public certificate or an authorization
//! boundary.  The ordinary-pair interval result is deliberately retained as
//! process-local proof material. Shared-feature relief and its exhaustive
//! whole-parent aggregation remain sealed in the same private boundary.

#[allow(
    dead_code,
    reason = "the sealed general-N proof is intentionally not a public authority"
)]
#[cfg(not(test))]
mod ordinary_interval;
#[allow(
    dead_code,
    reason = "the sealed general-N proof is intentionally not a public authority"
)]
#[cfg(test)]
pub(crate) mod ordinary_interval;

pub(crate) use ordinary_interval::public_adapter;
