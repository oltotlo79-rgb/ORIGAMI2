use super::{CayleyError, STAGE};

pub(super) fn set_fixed_counter(
    counter: &mut usize,
    required: usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), CayleyError> {
    if *counter != 0 {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    if required > maximum {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource,
        });
    }
    *counter = required;
    Ok(())
}

pub(super) fn charge_counter(
    counter: &mut usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), CayleyError> {
    let next = counter
        .checked_add(1)
        .ok_or(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource,
        })?;
    if next > maximum {
        return Err(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource,
        });
    }
    *counter = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{charge_counter, set_fixed_counter};
    use crate::cayley::{CayleyError, CayleyStage};

    #[test]
    fn fixed_counter_rejects_nonzero_before_resource_limit_without_mutation() {
        let mut counter = 7;
        assert_eq!(
            set_fixed_counter(&mut counter, 2, 1, "fixed"),
            Err(CayleyError::InvariantFailure {
                stage: CayleyStage::Containment,
            })
        );
        assert_eq!(counter, 7);
    }

    #[test]
    fn fixed_counter_resource_limit_preserves_zero() {
        let mut counter = 0;
        assert_eq!(
            set_fixed_counter(&mut counter, 2, 1, "fixed"),
            Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Containment,
                resource: "fixed",
            })
        );
        assert_eq!(counter, 0);
    }

    #[test]
    fn charged_counter_limit_and_overflow_leave_counter_unchanged() {
        let mut at_limit = 1;
        assert_eq!(
            charge_counter(&mut at_limit, 1, "charge"),
            Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Containment,
                resource: "charge",
            })
        );
        assert_eq!(at_limit, 1);

        let mut overflow = usize::MAX;
        assert_eq!(
            charge_counter(&mut overflow, usize::MAX, "overflow"),
            Err(CayleyError::ResourceLimitExceeded {
                stage: CayleyStage::Containment,
                resource: "overflow",
            })
        );
        assert_eq!(overflow, usize::MAX);
    }
}
