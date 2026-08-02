use std::{cmp::Reverse, collections::BinaryHeap};

use crate::OverlapCellKey;

/// The row representation for a canonical face triple `(a, b, c)`, where
/// `a < b < c` and the variables are `(a,b), (a,c), (b,c)`.
///
/// Rows `010` and `101` are precisely the two directed three-cycles.  Every
/// other row is a transitive order.
pub(crate) const TRANSITIVITY_ALLOWED_ROWS: [u8; 6] = [0, 1, 3, 4, 6, 7];

/// Compact representation of all `C(covering_faces.len(), 3)` transitivity
/// constraints supported by one overlap cell.
///
/// `pair_variables` is the upper triangle of `covering_faces`, in
/// lexicographic pair order.  It is deliberately O(ply^2): the solver can
/// stream every logical triple without allocating per-constraint vectors or
/// repeatedly searching the global overlap-pair registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransitivityConstraintFamily {
    pub(crate) covering_faces: Vec<usize>,
    pub(crate) pair_variables: Vec<usize>,
    pub(crate) supporting_cell: OverlapCellKey,
}

impl TransitivityConstraintFamily {
    pub(crate) fn logical_len(&self) -> Option<usize> {
        choose_three(self.covering_faces.len())
    }

    pub(super) fn pair_variable(&self, first: usize, second: usize) -> Option<usize> {
        self.pair_variables
            .get(pair_offset(self.covering_faces.len(), first, second))
            .copied()
    }

    fn constraint(&self, first: usize, second: usize, third: usize) -> TransitivityConstraint {
        debug_assert!(first < second && second < third);
        let face_count = self.covering_faces.len();
        let first_second = pair_offset(face_count, first, second);
        let first_third = pair_offset(face_count, first, third);
        let second_third = pair_offset(face_count, second, third);
        let variables = [
            self.pair_variables[first_second],
            self.pair_variables[first_third],
            self.pair_variables[second_third],
        ];
        debug_assert!(variables.windows(2).all(|pair| pair[0] < pair[1]));
        TransitivityConstraint {
            variables,
            faces: [
                self.covering_faces[first],
                self.covering_faces[second],
                self.covering_faces[third],
            ],
            supporting_cell: self.supporting_cell,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TransitivityConstraint {
    pub(crate) variables: [usize; 3],
    pub(crate) faces: [usize; 3],
    pub(crate) supporting_cell: OverlapCellKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TransitivityConstraints {
    families: Vec<TransitivityConstraintFamily>,
    logical_len: usize,
    maximum_ply: usize,
}

impl TransitivityConstraints {
    pub(crate) fn try_new(
        mut families: Vec<TransitivityConstraintFamily>,
        variable_count: usize,
    ) -> Option<Self> {
        let mut recomputed_len = 0_usize;
        let mut maximum_ply = 0_usize;
        for family in &families {
            if family.covering_faces.len() < 3
                || !family
                    .covering_faces
                    .windows(2)
                    .all(|faces| faces[0] < faces[1])
                || family.pair_variables.len() != choose_two(family.covering_faces.len())?
                || !family
                    .pair_variables
                    .windows(2)
                    .all(|variables| variables[0] < variables[1])
                || family
                    .pair_variables
                    .iter()
                    .any(|variable| *variable >= variable_count)
            {
                return None;
            }
            recomputed_len = recomputed_len.checked_add(family.logical_len()?)?;
            maximum_ply = maximum_ply.max(family.covering_faces.len());
        }
        families.sort_unstable_by_key(|family| family.supporting_cell.0);
        if families
            .windows(2)
            .any(|families| families[0].supporting_cell == families[1].supporting_cell)
        {
            return None;
        }
        // Equality between the independently rebuilt primary and verifier
        // problems must not depend on the arrangement traversal order.
        families.sort_unstable_by(|left, right| {
            left.covering_faces
                .cmp(&right.covering_faces)
                .then_with(|| left.supporting_cell.0.cmp(&right.supporting_cell.0))
                .then_with(|| left.pair_variables.cmp(&right.pair_variables))
        });
        Some(Self {
            families,
            logical_len: recomputed_len,
            maximum_ply,
        })
    }

    pub(crate) fn len(&self) -> usize {
        self.logical_len
    }

    pub(crate) fn family_count(&self) -> usize {
        self.families.len()
    }

    pub(crate) fn try_iter(&self) -> Result<TransitivityConstraintIter<'_>, ()> {
        TransitivityConstraintIter::try_new(&self.families)
    }

    pub(crate) fn iterator_working_memory_upper_bound(&self) -> Option<usize> {
        self.families
            .len()
            .checked_mul(std::mem::size_of::<Reverse<FamilyCursor>>())
    }

    pub(super) fn families(&self) -> &[TransitivityConstraintFamily] {
        &self.families
    }

    pub(super) fn maximum_ply(&self) -> usize {
        self.maximum_ply
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FamilyCursor {
    faces: [usize; 3],
    supporting_cell: [u8; 32],
    family: usize,
    first: usize,
    second: usize,
    third: usize,
}

impl FamilyCursor {
    fn first(family: usize, source: &TransitivityConstraintFamily) -> Option<Self> {
        (source.covering_faces.len() >= 3).then(|| Self {
            faces: [
                source.covering_faces[0],
                source.covering_faces[1],
                source.covering_faces[2],
            ],
            supporting_cell: source.supporting_cell.0,
            family,
            first: 0,
            second: 1,
            third: 2,
        })
    }

    fn advance(self, source: &TransitivityConstraintFamily) -> Option<Self> {
        let face_count = source.covering_faces.len();
        let (first, second, third) = if self.third + 1 < face_count {
            (self.first, self.second, self.third + 1)
        } else if self.second + 2 < face_count {
            (self.first, self.second + 1, self.second + 2)
        } else if self.first + 3 < face_count {
            (self.first + 1, self.first + 2, self.first + 3)
        } else {
            return None;
        };
        Some(Self {
            faces: [
                source.covering_faces[first],
                source.covering_faces[second],
                source.covering_faces[third],
            ],
            supporting_cell: source.supporting_cell.0,
            family: self.family,
            first,
            second,
            third,
        })
    }
}

pub(crate) struct TransitivityConstraintIter<'a> {
    families: &'a [TransitivityConstraintFamily],
    cursors: BinaryHeap<Reverse<FamilyCursor>>,
}

impl<'a> TransitivityConstraintIter<'a> {
    fn try_new(families: &'a [TransitivityConstraintFamily]) -> Result<Self, ()> {
        let mut cursors = Vec::new();
        cursors.try_reserve_exact(families.len()).map_err(|_| ())?;
        cursors.extend(
            families
                .iter()
                .enumerate()
                .filter_map(|(index, family)| FamilyCursor::first(index, family).map(Reverse)),
        );
        Ok(Self {
            families,
            cursors: BinaryHeap::from(cursors),
        })
    }
}

impl Iterator for TransitivityConstraintIter<'_> {
    type Item = TransitivityConstraint;

    fn next(&mut self) -> Option<Self::Item> {
        let Reverse(cursor) = self.cursors.pop()?;
        let family = &self.families[cursor.family];
        let constraint = family.constraint(cursor.first, cursor.second, cursor.third);
        if let Some(next) = cursor.advance(family) {
            self.cursors.push(Reverse(next));
        }
        Some(constraint)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

pub(crate) fn choose_two(value: usize) -> Option<usize> {
    if value < 2 {
        return Some(0);
    }
    value.checked_mul(value - 1)?.checked_div(2)
}

pub(crate) fn choose_three(value: usize) -> Option<usize> {
    if value < 3 {
        return Some(0);
    }
    value
        .checked_mul(value - 1)?
        .checked_mul(value - 2)?
        .checked_div(6)
}

fn pair_offset(face_count: usize, first: usize, second: usize) -> usize {
    debug_assert!(first < second && second < face_count);
    // Number of pairs in all preceding rows, plus the offset in this row.
    first * (2 * face_count - first - 1) / 2 + (second - first - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn family(faces: &[usize], cell: u8) -> TransitivityConstraintFamily {
        let pair_count = choose_two(faces.len()).expect("small fixture");
        TransitivityConstraintFamily {
            covering_faces: faces.to_vec(),
            pair_variables: (0..pair_count).collect(),
            supporting_cell: OverlapCellKey([cell; 32]),
        }
    }

    #[test]
    fn streams_each_small_family_in_expanded_constraint_order() {
        for ply in [3_usize, 4] {
            let faces = (10..10 + ply).collect::<Vec<_>>();
            let family = family(&faces, 7);
            let constraints = TransitivityConstraints::try_new(
                vec![family],
                choose_two(ply).expect("small fixture"),
            )
            .expect("valid fixture");
            let actual = constraints
                .try_iter()
                .expect("small iterator allocates")
                .collect::<Vec<_>>();
            let mut expected = Vec::new();
            for first in 0..ply {
                for second in first + 1..ply {
                    for third in second + 1..ply {
                        expected.push(TransitivityConstraint {
                            variables: [
                                pair_offset(ply, first, second),
                                pair_offset(ply, first, third),
                                pair_offset(ply, second, third),
                            ],
                            faces: [faces[first], faces[second], faces[third]],
                            supporting_cell: OverlapCellKey([7; 32]),
                        });
                    }
                }
            }
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn merges_families_by_faces_then_supporting_cell() {
        let constraints = TransitivityConstraints::try_new(
            vec![family(&[0, 1, 2, 4], 2), family(&[0, 1, 2, 3], 1)],
            6,
        )
        .expect("valid fixture");
        let keys = constraints
            .try_iter()
            .expect("small iterator allocates")
            .map(|constraint| (constraint.faces, constraint.supporting_cell.0[0]))
            .collect::<Vec<_>>();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted);
    }

    #[test]
    fn zero_and_malformed_families_are_fail_closed_without_iterator_panics() {
        let empty = TransitivityConstraints::try_new(Vec::new(), 0)
            .expect("zero logical transitivity constraints need no family storage");
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.family_count(), 0);
        assert_eq!(
            empty
                .try_iter()
                .expect("empty iterator needs no allocation")
                .count(),
            0
        );

        let malformed = [
            TransitivityConstraintFamily {
                covering_faces: vec![0, 0, 1],
                pair_variables: vec![0, 1, 2],
                supporting_cell: OverlapCellKey([1; 32]),
            },
            TransitivityConstraintFamily {
                covering_faces: vec![0, 1],
                pair_variables: vec![0],
                supporting_cell: OverlapCellKey([2; 32]),
            },
            TransitivityConstraintFamily {
                covering_faces: vec![0, 1, 2],
                pair_variables: vec![0, 1],
                supporting_cell: OverlapCellKey([3; 32]),
            },
            TransitivityConstraintFamily {
                covering_faces: vec![0, 1, 2],
                pair_variables: vec![0, 0, 2],
                supporting_cell: OverlapCellKey([4; 32]),
            },
            TransitivityConstraintFamily {
                covering_faces: vec![0, 1, 2],
                pair_variables: vec![0, 1, 3],
                supporting_cell: OverlapCellKey([5; 32]),
            },
        ];
        for family in malformed {
            assert!(TransitivityConstraints::try_new(vec![family], 3).is_none());
        }

        let duplicate_cell = OverlapCellKey([8; 32]);
        assert!(
            TransitivityConstraints::try_new(
                vec![
                    TransitivityConstraintFamily {
                        covering_faces: vec![0, 1, 2],
                        pair_variables: vec![0, 1, 2],
                        supporting_cell: duplicate_cell,
                    },
                    TransitivityConstraintFamily {
                        covering_faces: vec![0, 1, 3],
                        pair_variables: vec![0, 1, 2],
                        supporting_cell: duplicate_cell,
                    },
                ],
                3,
            )
            .is_none()
        );
    }
}
