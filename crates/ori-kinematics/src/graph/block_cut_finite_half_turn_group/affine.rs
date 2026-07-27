use std::collections::HashMap;

use num_rational::BigRational;
use num_traits::{One, Zero};

use super::super::exact_generator_word::{CanonicalInfiniteLineV1, exact_plucker_components_v1};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ExactHalfTurnAffineV1 {
    rotation: [[BigRational; 3]; 3],
    translation: [BigRational; 3],
}

impl ExactHalfTurnAffineV1 {
    pub(super) fn identity() -> Self {
        Self {
            rotation: std::array::from_fn(|row| {
                std::array::from_fn(|column| {
                    if row == column {
                        BigRational::one()
                    } else {
                        BigRational::zero()
                    }
                })
            }),
            translation: std::array::from_fn(|_| BigRational::zero()),
        }
    }

    pub(super) fn from_line(line: &CanonicalInfiniteLineV1) -> Option<Self> {
        let (direction, moment) = exact_plucker_components_v1(line)?;
        let norm = direction
            .iter()
            .map(|value| value * value)
            .sum::<BigRational>();
        if norm.is_zero() {
            return None;
        }
        let incidence = direction
            .iter()
            .zip(&moment)
            .map(|(direction, moment)| direction * moment)
            .sum::<BigRational>();
        if !incidence.is_zero() {
            return None;
        }
        let cross = [
            &direction[1] * &moment[2] - &direction[2] * &moment[1],
            &direction[2] * &moment[0] - &direction[0] * &moment[2],
            &direction[0] * &moment[1] - &direction[1] * &moment[0],
        ];
        let closest = cross.map(|value| value / &norm);
        let two = BigRational::from_integer(2.into());
        let rotation = std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                let projected = &two * &direction[row] * &direction[column] / &norm;
                if row == column {
                    projected - BigRational::one()
                } else {
                    projected
                }
            })
        });
        let translation = closest.map(|value| &two * value);
        Some(Self {
            rotation,
            translation,
        })
    }

    pub(super) fn right_product(&self, right: &Self) -> Self {
        let rotation = std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                (0..3)
                    .map(|inner| &self.rotation[row][inner] * &right.rotation[inner][column])
                    .sum()
            })
        });
        let translation = std::array::from_fn(|row| {
            &self.translation[row]
                + (0..3)
                    .map(|inner| &self.rotation[row][inner] * &right.translation[inner])
                    .sum::<BigRational>()
        });
        Self {
            rotation,
            translation,
        }
    }

    fn storage_bits(&self, maximum_component_bits: u64) -> Option<usize> {
        self.rotation
            .iter()
            .flatten()
            .chain(self.translation.iter())
            .try_fold(0usize, |total, component| {
                let numerator = component.numer().bits();
                let denominator = component.denom().bits();
                if numerator > maximum_component_bits || denominator > maximum_component_bits {
                    return None;
                }
                total
                    .checked_add(usize::try_from(numerator).ok()?)
                    .and_then(|value| value.checked_add(usize::try_from(denominator).ok()?))
            })
    }

    #[cfg(test)]
    pub(super) fn translation(&self) -> &[BigRational; 3] {
        &self.translation
    }

    #[cfg(test)]
    pub(super) fn apply_point(&self, point: &[BigRational; 3]) -> [BigRational; 3] {
        std::array::from_fn(|row| {
            &self.translation[row]
                + (0..3)
                    .map(|column| &self.rotation[row][column] * &point[column])
                    .sum::<BigRational>()
        })
    }
}

#[derive(Debug)]
pub(super) struct FiniteHalfTurnGroupV1 {
    pub(super) order: usize,
    pub(super) carrier_count: usize,
    pub(super) transitions: Vec<usize>,
    pub(super) products: usize,
    pub(super) exact_storage_bits: usize,
    pub(super) exact_work_bits: usize,
}

impl FiniteHalfTurnGroupV1 {
    pub(super) fn transition(&self, state: usize, carrier: usize) -> Option<usize> {
        if state >= self.order || carrier >= self.carrier_count {
            return None;
        }
        self.transitions
            .get(
                state
                    .checked_mul(self.carrier_count)?
                    .checked_add(carrier)?,
            )
            .copied()
    }
}

pub(super) fn enumerate_finite_half_turn_group_v1(
    carriers: &[CanonicalInfiniteLineV1],
    maximum_order: usize,
    maximum_products: usize,
    maximum_exact_storage_bits: usize,
    maximum_exact_work_bits: usize,
    maximum_component_bits: u64,
) -> Option<FiniteHalfTurnGroupV1> {
    if carriers.is_empty() || maximum_order < 2 || carriers.len() > maximum_order {
        return None;
    }
    let transition_capacity = maximum_order.checked_mul(carriers.len())?;
    if transition_capacity > maximum_products {
        return None;
    }

    let mut generators = Vec::new();
    generators.try_reserve_exact(carriers.len()).ok()?;
    let mut generator_bits = 0usize;
    for carrier in carriers {
        let generator = ExactHalfTurnAffineV1::from_line(carrier)?;
        generator_bits =
            generator_bits.checked_add(generator.storage_bits(maximum_component_bits)?)?;
        generators.push(generator);
    }
    if generator_bits > maximum_exact_storage_bits {
        return None;
    }

    let identity = ExactHalfTurnAffineV1::identity();
    let mut exact_work_bits = 0usize;
    for generator in &generators {
        let square = generator.right_product(generator);
        exact_work_bits =
            exact_work_bits.checked_add(square.storage_bits(maximum_component_bits)?)?;
        if exact_work_bits > maximum_exact_work_bits || square != identity {
            return None;
        }
    }
    let identity_bits = identity.storage_bits(maximum_component_bits)?;
    let mut elements = Vec::new();
    elements.try_reserve_exact(maximum_order).ok()?;
    elements.push(identity.clone());
    let mut by_element = HashMap::new();
    by_element.try_reserve(maximum_order).ok()?;
    by_element.insert(identity, 0usize);
    let mut element_bits = identity_bits;
    let mut transitions = Vec::new();
    transitions.try_reserve_exact(transition_capacity).ok()?;
    let mut state = 0usize;
    let mut products = 0usize;
    while state < elements.len() {
        for generator in &generators {
            products = products.checked_add(1)?;
            if products > maximum_products {
                return None;
            }
            let candidate = elements.get(state)?.right_product(generator);
            let candidate_bits = candidate.storage_bits(maximum_component_bits)?;
            exact_work_bits = exact_work_bits.checked_add(candidate_bits)?;
            if exact_work_bits > maximum_exact_work_bits {
                return None;
            }
            let next = if let Some(existing) = by_element.get(&candidate) {
                *existing
            } else {
                if elements.len() >= maximum_order {
                    return None;
                }
                let next_bits = element_bits.checked_add(candidate_bits)?;
                let peak_bits = next_bits.checked_mul(2)?.checked_add(generator_bits)?;
                if peak_bits > maximum_exact_storage_bits {
                    return None;
                }
                let index = elements.len();
                if by_element.insert(candidate.clone(), index).is_some() {
                    return None;
                }
                elements.push(candidate);
                element_bits = next_bits;
                index
            };
            transitions.push(next);
        }
        state = state.checked_add(1)?;
    }
    let expected_products = elements.len().checked_mul(generators.len())?;
    let exact_storage_bits = element_bits.checked_mul(2)?.checked_add(generator_bits)?;
    for state in 0..elements.len() {
        for carrier in 0..generators.len() {
            let first =
                *transitions.get(state.checked_mul(generators.len())?.checked_add(carrier)?)?;
            let second =
                *transitions.get(first.checked_mul(generators.len())?.checked_add(carrier)?)?;
            if second != state {
                return None;
            }
        }
    }
    (products == expected_products
        && transitions.len() == expected_products
        && exact_storage_bits <= maximum_exact_storage_bits)
        .then_some(FiniteHalfTurnGroupV1 {
            order: elements.len(),
            carrier_count: generators.len(),
            transitions,
            products,
            exact_storage_bits,
            exact_work_bits,
        })
}
