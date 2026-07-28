#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct CardinalRotationV1 {
    matrix: [[i8; 3]; 3],
}

impl CardinalRotationV1 {
    pub(super) const fn identity() -> Self {
        Self {
            matrix: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
        }
    }

    pub(super) fn quarter_turn(axis: usize, turns: i8) -> Option<Self> {
        let turns = i16::from(turns).rem_euclid(4);
        let matrix = match (axis, turns) {
            (_, 0) if axis < 3 => Self::identity().matrix,
            (0, 1) => [[1, 0, 0], [0, 0, -1], [0, 1, 0]],
            (0, 2) => [[1, 0, 0], [0, -1, 0], [0, 0, -1]],
            (0, 3) => [[1, 0, 0], [0, 0, 1], [0, -1, 0]],
            (1, 1) => [[0, 0, 1], [0, 1, 0], [-1, 0, 0]],
            (1, 2) => [[-1, 0, 0], [0, 1, 0], [0, 0, -1]],
            (1, 3) => [[0, 0, -1], [0, 1, 0], [1, 0, 0]],
            (2, 1) => [[0, -1, 0], [1, 0, 0], [0, 0, 1]],
            (2, 2) => [[-1, 0, 0], [0, -1, 0], [0, 0, 1]],
            (2, 3) => [[0, 1, 0], [-1, 0, 0], [0, 0, 1]],
            _ => return None,
        };
        let rotation = Self { matrix };
        rotation.is_valid().then_some(rotation)
    }

    pub(super) fn right_product(self, right: Self) -> Option<Self> {
        let mut matrix = [[0i8; 3]; 3];
        for (row, output_row) in matrix.iter_mut().enumerate() {
            for (column, output) in output_row.iter_mut().enumerate() {
                let mut entry = 0i16;
                for inner in 0..3 {
                    entry = entry.checked_add(
                        i16::from(self.matrix[row][inner])
                            .checked_mul(i16::from(right.matrix[inner][column]))?,
                    )?;
                }
                *output = i8::try_from(entry).ok()?;
            }
        }
        let product = Self { matrix };
        product.is_valid().then_some(product)
    }

    pub(super) fn inverse(self) -> Option<Self> {
        if !self.is_valid() {
            return None;
        }
        let matrix =
            std::array::from_fn(|row| std::array::from_fn(|column| self.matrix[column][row]));
        let inverse = Self { matrix };
        inverse.is_valid().then_some(inverse)
    }

    pub(super) fn is_valid(self) -> bool {
        if self
            .matrix
            .iter()
            .flatten()
            .any(|entry| !matches!(*entry, -1..=1))
        {
            return false;
        }
        let rows_are_signed_basis = self.matrix.iter().all(|row| {
            row.iter().filter(|entry| **entry != 0).count() == 1
                && row.iter().map(|entry| i16::from(*entry).abs()).sum::<i16>() == 1
        });
        let columns_are_signed_basis = (0..3).all(|column| {
            self.matrix.iter().filter(|row| row[column] != 0).count() == 1
                && self
                    .matrix
                    .iter()
                    .map(|row| i16::from(row[column]).abs())
                    .sum::<i16>()
                    == 1
        });
        let determinant = i16::from(self.matrix[0][0])
            * (i16::from(self.matrix[1][1]) * i16::from(self.matrix[2][2])
                - i16::from(self.matrix[1][2]) * i16::from(self.matrix[2][1]))
            - i16::from(self.matrix[0][1])
                * (i16::from(self.matrix[1][0]) * i16::from(self.matrix[2][2])
                    - i16::from(self.matrix[1][2]) * i16::from(self.matrix[2][0]))
            + i16::from(self.matrix[0][2])
                * (i16::from(self.matrix[1][0]) * i16::from(self.matrix[2][1])
                    - i16::from(self.matrix[1][1]) * i16::from(self.matrix[2][0]));
        rows_are_signed_basis && columns_are_signed_basis && determinant == 1
    }

    #[cfg(test)]
    pub(super) const fn from_matrix_for_test(matrix: [[i8; 3]; 3]) -> Self {
        Self { matrix }
    }
}
