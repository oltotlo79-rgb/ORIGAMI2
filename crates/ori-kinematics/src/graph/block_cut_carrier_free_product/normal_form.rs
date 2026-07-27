use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CarrierSyllableV1 {
    prefix: usize,
    carrier: usize,
    exponents: Arc<[i32]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NormalFormUsageV1 {
    pub(super) directed_appends: usize,
    pub(super) vector_work: usize,
    pub(super) retained_vector_storage: usize,
    pub(super) node_storage: usize,
}

pub(super) struct CarrierFreeProductInternerV1 {
    profile_count: usize,
    node_limit: usize,
    nodes: Vec<Option<CarrierSyllableV1>>,
    by_syllable: HashMap<CarrierSyllableV1, usize>,
    usage: NormalFormUsageV1,
}

impl CarrierFreeProductInternerV1 {
    pub(super) fn prepare(profile_count: usize, node_limit: usize) -> Option<Self> {
        if profile_count == 0 || node_limit == 0 {
            return None;
        }
        let extension_limit = node_limit.checked_sub(1)?;
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(node_limit).ok()?;
        nodes.push(None);
        let mut by_syllable = HashMap::new();
        by_syllable.try_reserve(extension_limit).ok()?;
        Some(Self {
            profile_count,
            node_limit,
            nodes,
            by_syllable,
            usage: NormalFormUsageV1 {
                directed_appends: 0,
                vector_work: 0,
                retained_vector_storage: 0,
                node_storage: 1,
            },
        })
    }

    pub(super) fn append(
        &mut self,
        word: usize,
        carrier: usize,
        profile: usize,
        sign: i8,
    ) -> Option<usize> {
        if profile >= self.profile_count || !matches!(sign, -1 | 1) {
            return None;
        }
        let current = self.nodes.get(word)?.as_ref();
        let (prefix, source) = if current.is_some_and(|node| node.carrier == carrier) {
            let node = current?;
            (node.prefix, Some(node.exponents.as_ref()))
        } else {
            (word, None)
        };

        let mut exponents = Vec::new();
        exponents.try_reserve_exact(self.profile_count).ok()?;
        for coordinate in 0..self.profile_count {
            self.usage.vector_work = self.usage.vector_work.checked_add(1)?;
            exponents.push(match source {
                Some(values) => *values.get(coordinate)?,
                None => 0,
            });
        }
        self.usage.directed_appends = self.usage.directed_appends.checked_add(1)?;
        let coordinate = exponents.get_mut(profile)?;
        *coordinate = (*coordinate).checked_add(i32::from(sign))?;
        if exponents.iter().all(|value| *value == 0) {
            return Some(prefix);
        }
        if prefix != 0
            && self
                .nodes
                .get(prefix)?
                .as_ref()
                .is_none_or(|node| node.carrier == carrier)
        {
            return None;
        }

        let syllable = CarrierSyllableV1 {
            prefix,
            carrier,
            exponents: Arc::from(exponents.into_boxed_slice()),
        };
        if let Some(existing) = self.by_syllable.get(&syllable) {
            return Some(*existing);
        }
        if self.nodes.len() >= self.node_limit {
            return None;
        }
        let index = self.nodes.len();
        self.usage.retained_vector_storage = self
            .usage
            .retained_vector_storage
            .checked_add(self.profile_count)?;
        self.usage.node_storage = self.usage.node_storage.checked_add(1)?;
        self.nodes.push(Some(syllable.clone()));
        if self.by_syllable.insert(syllable, index).is_some() {
            return None;
        }
        Some(index)
    }

    pub(super) fn usage(&self) -> NormalFormUsageV1 {
        self.usage
    }

    pub(super) fn invariant_holds(&self) -> bool {
        if self.nodes.is_empty()
            || self.nodes[0].is_some()
            || self.nodes.len() != self.usage.node_storage
            || self.by_syllable.len().checked_add(1) != Some(self.nodes.len())
            || self.usage.retained_vector_storage
                != self
                    .nodes
                    .len()
                    .checked_sub(1)
                    .and_then(|nodes| nodes.checked_mul(self.profile_count))
                    .unwrap_or(usize::MAX)
        {
            return false;
        }
        self.nodes.iter().enumerate().skip(1).all(|(index, node)| {
            node.as_ref().is_some_and(|node| {
                node.prefix < index
                    && node.exponents.len() == self.profile_count
                    && node.exponents.iter().any(|value| *value != 0)
                    && (node.prefix == 0
                        || self.nodes[node.prefix]
                            .as_ref()
                            .is_some_and(|prefix| prefix.carrier != node.carrier))
                    && self.by_syllable.get(node) == Some(&index)
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_form_merges_only_adjacent_equal_carriers_and_exposes_prefix() {
        let mut words = CarrierFreeProductInternerV1::prepare(2, 64).unwrap();
        let a_p = words.append(0, 0, 0, 1).unwrap();
        let a_pq = words.append(a_p, 0, 1, 1).unwrap();
        let a_q = words.append(a_pq, 0, 0, -1).unwrap();
        let identity = words.append(a_q, 0, 1, -1).unwrap();
        assert_eq!(identity, 0);

        let a_p = words.append(0, 0, 0, 1).unwrap();
        let a_p_b = words.append(a_p, 1, 0, 1).unwrap();
        let exposed_a_p = words.append(a_p_b, 1, 0, -1).unwrap();
        assert_eq!(exposed_a_p, a_p);
        assert!(words.invariant_holds());
        assert_eq!(
            words.usage(),
            NormalFormUsageV1 {
                directed_appends: 7,
                vector_work: 14,
                retained_vector_storage: 8,
                node_storage: 5,
            }
        );
    }

    #[test]
    fn normal_form_keeps_a_different_carrier_commutator_nontrivial() {
        let mut words = CarrierFreeProductInternerV1::prepare(1, 64).unwrap();
        let a = words.append(0, 0, 0, 1).unwrap();
        let ab = words.append(a, 1, 0, 1).unwrap();
        let aba_inverse = words.append(ab, 0, 0, -1).unwrap();
        let commutator = words.append(aba_inverse, 1, 0, -1).unwrap();
        assert_ne!(commutator, 0);
        assert!(words.invariant_holds());
    }
}
