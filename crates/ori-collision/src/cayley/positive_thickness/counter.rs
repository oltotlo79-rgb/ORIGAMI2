use super::{CayleyError, STAGE, checked_work_sum};

pub(super) fn check_resource_limit(
    actual: usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), CayleyError> {
    if actual > maximum {
        Err(CayleyError::ResourceLimitExceeded {
            stage: STAGE,
            resource,
        })
    } else {
        Ok(())
    }
}

pub(super) fn set_fixed_counter(
    counter: &mut usize,
    required: usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), CayleyError> {
    if *counter != 0 {
        return Err(CayleyError::InvariantFailure { stage: STAGE });
    }
    check_resource_limit(required, maximum, resource)?;
    *counter = required;
    Ok(())
}

pub(super) fn charge_counter(
    counter: &mut usize,
    maximum: usize,
    resource: &'static str,
) -> Result<(), CayleyError> {
    let next = checked_work_sum(*counter, 1, STAGE, resource)?;
    check_resource_limit(next, maximum, resource)?;
    *counter = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{charge_counter, check_resource_limit, set_fixed_counter};
    use crate::cayley::{CayleyError, CayleyStage};

    #[test]
    fn resource_limit_and_counters_accept_the_exact_boundary() {
        assert_eq!(check_resource_limit(2, 2, "boundary"), Ok(()));

        let mut fixed = 0;
        set_fixed_counter(&mut fixed, 2, 2, "fixed").unwrap();
        assert_eq!(fixed, 2);

        let mut charged = 1;
        charge_counter(&mut charged, 2, "charged").unwrap();
        assert_eq!(charged, 2);
    }

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
