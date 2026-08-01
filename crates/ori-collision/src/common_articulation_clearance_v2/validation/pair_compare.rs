//! Interruptible equality checks for retained canonical pair registries.

use super::*;

/// Compares a retained canonical pair registry without an uninterruptible
/// `Vec` equality operation.  A stop is checked both immediately before each
/// observed pair and immediately before reporting a mismatch, so a concurrent
/// cancellation/deadline always wins over a stale-binding diagnostic.
pub(crate) fn cross_block_pairs_equal_with_checkpoint_v2(
    retained: &[CommonArticulationCrossBlockFacePairV2],
    candidate: &[CommonArticulationCrossBlockFacePairV2],
    checkpoint: &mut impl FnMut() -> Result<(), CommonArticulationClearanceStopV2>,
) -> Result<bool, CommonArticulationClearanceErrorV2> {
    checkpoint_v2(checkpoint)?;
    if retained.len() != candidate.len() {
        checkpoint_v2(checkpoint)?;
        return Ok(false);
    }
    for (retained, candidate) in retained.iter().zip(candidate) {
        checkpoint_v2(checkpoint)?;
        if retained != candidate {
            checkpoint_v2(checkpoint)?;
            return Ok(false);
        }
    }
    checkpoint_v2(checkpoint)?;
    Ok(true)
}
