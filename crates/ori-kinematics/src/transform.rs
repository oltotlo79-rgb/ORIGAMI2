use crate::KinematicsError;

const DEGREES_TO_RADIANS: f64 = 0.017_453_292_519_943_295;

/// One finite point or free vector in a caller-selected Cartesian frame.
///
/// The fields are private so non-finite coordinates cannot inhabit this type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point3 {
    x: f64,
    y: f64,
    z: f64,
}

impl Point3 {
    pub(crate) const ORIGIN: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    /// Creates a finite point and canonicalizes every signed zero.
    pub fn new(x: f64, y: f64, z: f64) -> Result<Self, KinematicsError> {
        if ![x, y, z].into_iter().all(f64::is_finite) {
            return Err(KinematicsError::UnrepresentableGeometry);
        }
        Ok(Self {
            x: canonical_zero(x),
            y: canonical_zero(y),
            z: canonical_zero(z),
        })
    }

    #[must_use]
    pub const fn x(self) -> f64 {
        self.x
    }

    #[must_use]
    pub const fn y(self) -> f64 {
        self.y
    }

    #[must_use]
    pub const fn z(self) -> f64 {
        self.z
    }
}

/// A finite read-only rigid transform.
///
/// There is deliberately no public raw-matrix constructor. Instances are
/// issued only by the native kinematics solver or by [`Self::identity`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidTransform {
    rotation: [[f64; 3]; 3],
    translation: Point3,
}

impl RigidTransform {
    const IDENTITY: Self = Self {
        rotation: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        translation: Point3::ORIGIN,
    };

    /// Returns the unique identity transform.
    #[must_use]
    pub const fn identity() -> Self {
        Self::IDENTITY
    }

    /// Returns a detached row-major copy for read-only observation.
    #[must_use]
    pub const fn rotation_rows(self) -> [[f64; 3]; 3] {
        self.rotation
    }

    /// Returns the finite translation component.
    #[must_use]
    pub const fn translation(self) -> Point3 {
        self.translation
    }

    /// Applies the transform to a finite point.
    pub fn apply_point(self, point: Point3) -> Result<Point3, KinematicsError> {
        add(self.rotate_vector(point)?, self.translation)
    }

    /// Applies only the rotation component to a finite vector.
    pub fn apply_vector(self, vector: Point3) -> Result<Point3, KinematicsError> {
        self.rotate_vector(vector)
    }

    /// Applies the inverse rigid transform to a finite world-space point.
    ///
    /// Prepared transforms are orthonormal, so the inverse rotation is the
    /// transpose. This observer is used by authenticated native workflows that
    /// must map a certified world-space feature back to material coordinates.
    pub fn inverse_apply_point(self, point: Point3) -> Result<Point3, KinematicsError> {
        let shifted = subtract(point, self.translation)?;
        Point3::new(
            self.rotation[0][0] * shifted.x
                + self.rotation[1][0] * shifted.y
                + self.rotation[2][0] * shifted.z,
            self.rotation[0][1] * shifted.x
                + self.rotation[1][1] * shifted.y
                + self.rotation[2][1] * shifted.z,
            self.rotation[0][2] * shifted.x
                + self.rotation[1][2] * shifted.y
                + self.rotation[2][2] * shifted.z,
        )
    }

    /// Returns the world-space rigid motion that maps points transformed by
    /// `initial` to the same material points transformed by `self`.
    pub fn relative_to(self, initial: Self) -> Result<Self, KinematicsError> {
        let rotation = initial.rotation;
        let inverse_rotation = [
            [rotation[0][0], rotation[1][0], rotation[2][0]],
            [rotation[0][1], rotation[1][1], rotation[2][1]],
            [rotation[0][2], rotation[1][2], rotation[2][2]],
        ];
        let inverse_translation = Point3::new(
            -(inverse_rotation[0][0] * initial.translation.x
                + inverse_rotation[0][1] * initial.translation.y
                + inverse_rotation[0][2] * initial.translation.z),
            -(inverse_rotation[1][0] * initial.translation.x
                + inverse_rotation[1][1] * initial.translation.y
                + inverse_rotation[1][2] * initial.translation.z),
            -(inverse_rotation[2][0] * initial.translation.x
                + inverse_rotation[2][1] * initial.translation.y
                + inverse_rotation[2][2] * initial.translation.z),
        )?;
        self.compose(finite_transform(Self {
            rotation: inverse_rotation,
            translation: inverse_translation,
        })?)
    }

    pub(crate) fn around_axis(
        point: Point3,
        axis: Point3,
        angle_degrees: f64,
    ) -> Result<Self, KinematicsError> {
        let (sine, cosine) = deterministic_sin_cos_degrees(angle_degrees)?;
        let one_minus = 1.0 - cosine;
        let (x, y, z) = (axis.x, axis.y, axis.z);
        let rotation = [
            [
                cosine + x * x * one_minus,
                x * y * one_minus - z * sine,
                x * z * one_minus + y * sine,
            ],
            [
                y * x * one_minus + z * sine,
                cosine + y * y * one_minus,
                y * z * one_minus - x * sine,
            ],
            [
                z * x * one_minus - y * sine,
                z * y * one_minus + x * sine,
                cosine + z * z * one_minus,
            ],
        ];
        let rotated_point = rotate_matrix(rotation, point)?;
        let translation = subtract(point, rotated_point)?;
        finite_transform(Self {
            rotation,
            translation,
        })
    }

    pub(crate) fn compose(self, local: Self) -> Result<Self, KinematicsError> {
        let mut rotation = [[0.0; 3]; 3];
        for (row, target_row) in rotation.iter_mut().enumerate() {
            for (column, target) in target_row.iter_mut().enumerate() {
                *target = (0..3)
                    .map(|index| self.rotation[row][index] * local.rotation[index][column])
                    .sum();
            }
        }
        let translation = add(self.rotate_vector(local.translation)?, self.translation)?;
        finite_transform(Self {
            rotation,
            translation,
        })
    }

    fn rotate_vector(self, vector: Point3) -> Result<Point3, KinematicsError> {
        rotate_matrix(self.rotation, vector)
    }
}

/// Deterministic sine and cosine for the signed angle range used internally.
///
/// Cardinal results are represented exactly; other angles use the pinned
/// `libm` implementation used by portable instruction export.
pub fn deterministic_sin_cos_degrees(angle_degrees: f64) -> Result<(f64, f64), KinematicsError> {
    if !angle_degrees.is_finite() || !(-180.0..=180.0).contains(&angle_degrees) {
        return Err(KinematicsError::UnrepresentableGeometry);
    }
    let (sine, cosine) = match canonical_zero(angle_degrees) {
        0.0 => (0.0, 1.0),
        90.0 => (1.0, 0.0),
        -90.0 => (-1.0, 0.0),
        180.0 | -180.0 => (0.0, -1.0),
        angle => libm::sincos(angle * DEGREES_TO_RADIANS),
    };
    if sine.is_finite() && cosine.is_finite() {
        Ok((canonical_zero(sine), canonical_zero(cosine)))
    } else {
        Err(KinematicsError::UnrepresentableGeometry)
    }
}

pub(crate) const fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

pub(crate) fn add(first: Point3, second: Point3) -> Result<Point3, KinematicsError> {
    Point3::new(first.x + second.x, first.y + second.y, first.z + second.z)
}

pub(crate) fn subtract(first: Point3, second: Point3) -> Result<Point3, KinematicsError> {
    Point3::new(first.x - second.x, first.y - second.y, first.z - second.z)
}

pub(crate) fn scale(value: Point3, scalar: f64) -> Result<Point3, KinematicsError> {
    Point3::new(value.x * scalar, value.y * scalar, value.z * scalar)
}

pub(crate) fn length(value: Point3) -> Result<f64, KinematicsError> {
    let length = dot(value, value).sqrt();
    if length.is_finite() && length > 0.0 {
        Ok(length)
    } else {
        Err(KinematicsError::UnrepresentableGeometry)
    }
}

fn dot(first: Point3, second: Point3) -> f64 {
    first.x * second.x + first.y * second.y + first.z * second.z
}

fn rotate_matrix(matrix: [[f64; 3]; 3], point: Point3) -> Result<Point3, KinematicsError> {
    Point3::new(
        matrix[0][0] * point.x + matrix[0][1] * point.y + matrix[0][2] * point.z,
        matrix[1][0] * point.x + matrix[1][1] * point.y + matrix[1][2] * point.z,
        matrix[2][0] * point.x + matrix[2][1] * point.y + matrix[2][2] * point.z,
    )
}

fn finite_transform(value: RigidTransform) -> Result<RigidTransform, KinematicsError> {
    value
        .rotation
        .into_iter()
        .flatten()
        .chain([
            value.translation.x,
            value.translation.y,
            value.translation.z,
        ])
        .all(f64::is_finite)
        .then_some(value)
        .ok_or(KinematicsError::UnrepresentableGeometry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_apply_point_round_trips_cardinal_rigid_transforms_bit_exactly() {
        let axis_origin = Point3::new(2.0, 3.0, 4.0).unwrap();
        let axis = Point3::new(0.0, 0.0, 1.0).unwrap();
        let transform = RigidTransform::around_axis(axis_origin, axis, 90.0).unwrap();
        for source in [
            axis_origin,
            Point3::new(7.0, -5.0, 11.0).unwrap(),
            Point3::new(-13.0, 17.0, -19.0).unwrap(),
        ] {
            let world = transform.apply_point(source).unwrap();
            let restored = transform.inverse_apply_point(world).unwrap();
            assert_eq!(restored, source);
        }
    }

    #[test]
    fn inverse_apply_point_rejects_non_finite_results() {
        let transform = RigidTransform {
            rotation: [[f64::MAX, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            translation: Point3::ORIGIN,
        };
        assert_eq!(
            transform.inverse_apply_point(Point3::new(f64::MAX, 0.0, 0.0).unwrap()),
            Err(KinematicsError::UnrepresentableGeometry)
        );
    }

    #[test]
    fn relative_transform_maps_initial_world_points_to_current_world_points() {
        let axis = Point3::new(0.0, 0.0, 1.0).unwrap();
        let initial =
            RigidTransform::around_axis(Point3::new(3.0, 0.0, 0.0).unwrap(), axis, 180.0).unwrap();
        let current =
            RigidTransform::around_axis(Point3::new(5.0, 0.0, 0.0).unwrap(), axis, 90.0).unwrap();
        let relative = current.relative_to(initial).unwrap();
        let material = Point3::new(7.0, 11.0, 13.0).unwrap();
        let initial_world = initial.apply_point(material).unwrap();
        let current_world = current.apply_point(material).unwrap();
        let mapped = relative.apply_point(initial_world).unwrap();
        assert!((mapped.x() - current_world.x()).abs() <= 1.0e-12);
        assert!((mapped.y() - current_world.y()).abs() <= 1.0e-12);
        assert!((mapped.z() - current_world.z()).abs() <= 1.0e-12);
    }
}
