use num_traits::Float;
use std::ops::*;
use crate::math::fast_inv_sqrt64;
use std::fmt::{self, Display, Formatter};

/// Represents a vector in 3D space. Note that this coordinate system is consistent with that used
/// by Minecraft which has the y and z axes flipped.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Hash, Debug)]
pub struct Vector<T> {
    /// The x component of the vector.
    pub x: T,
    /// The y component of the vector.
    pub y: T,
    /// The z component of the vector.
    pub z: T
}

impl Vector<f32> {
    /// Returns the zero vector, or the vector with all components equaling zero.
    pub const fn zero() -> Self {
        Vector {
            x: 0.0,
            y: 0.0,
            z: 0.0
        }
    }
}

impl Vector<f64> {
    /// Returns the zero vector, or the vector with all components equaling zero.
    pub const fn zero() -> Self {
        Vector {
            x: 0.0,
            y: 0.0,
            z: 0.0
        }
    }

    /// Computes the angle with the given vector, trading some accuracy for efficiency.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// let u = Vector::<f64>::rect(1.0, 0.0, 1.0);
    /// let v = Vector::<f64>::rect(1.0, 2.0f64.sqrt(), 1.0);
    /// assert!((u.fast_angle(&v) - std::f64::consts::FRAC_PI_4).abs() < 0.01);
    /// ```
    pub fn fast_angle(&self, other: &Self) -> f64 {
        // TODO: switch to f64::clamp when stable
        (self.dot(other) * fast_inv_sqrt64(self.len_sq() * other.len_sq())).min(1.0).max(-1.0).acos()
    }

    /// Normalizes this vector, trading some accuracy for efficiency.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// use std::f64::consts::*;
    /// let mut u = Vector::rect(LN_2, -PI, E);
    /// u.fast_normalize();
    /// assert!((u.len_sq() - 1.0).abs() < 0.01);
    /// ```
    pub fn fast_normalize(&mut self) {
        *self *= fast_inv_sqrt64(self.len_sq());
    }

    /// Copies this vector and normalizes the copy, returning the normalized vector, trading some accuracy
    /// for efficiency.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// use std::f64::consts::*;
    /// let u = Vector::rect(LN_2, -PI, E);
    /// assert!((u.fast_normalized().len_sq() - 1.0).abs() < 0.01);
    /// ```
    pub fn fast_normalized(self) -> Self {
        let mut copy = self;
        copy.fast_normalize();
        copy
    }
}

impl<T: Float + Copy> Vector<T> {
    /// Creates a new vector with all components equaling zero.
    pub fn new() -> Self {
        Vector {
            x: T::zero(),
            y: T::zero(),
            z: T::zero()
        }
    }

    /// Creates a vector using rectangular coordinates.
    pub fn rect(x: T, y: T, z: T) -> Self {
        Vector { x, y, z }
    }

    /// Creates a vector using spherical coordinates.
    pub fn spher(radius: T, theta: T, phi: T) -> Self {
        let r_proj_xz = radius * phi.sin();

        Vector {
            x: r_proj_xz * theta.cos(),
            y: radius * phi.cos(),
            z: r_proj_xz * theta.sin()
        }
    }

    /// Creates a vector using a length (radius) and two angles about principal axes: yaw and pitch. Yaw
    /// is a measure of clockwise rotation about the y axis from the positive z axis. Pitch is a measure
    /// of the angle below the x-z plane.
    pub fn principal_axes(radius: T, yaw: T, pitch: T) -> Self {
        let r_proj_xz = radius * pitch.cos();

        Vector {
            x: -r_proj_xz * yaw.sin(),
            y: -radius * pitch.sin(),
            z: r_proj_xz * yaw.cos()
        }
    }

    /// Computes the angle between this vector and the given vector (in radians).
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// use std::f64::consts::{SQRT_2, FRAC_PI_4};
    /// let u = Vector::rect(1.0, 0.0, 1.0);
    /// let v = Vector::rect(1.0, SQRT_2, 1.0);
    /// assert!((u.angle(&v) - FRAC_PI_4).abs() < 1e-10);
    /// ```
    pub fn angle(&self, other: &Self) -> T {
        (self.dot(other) / (self.len_sq() * other.len_sq()).sqrt()).min(T::one()).max(-T::one()).acos()
    }

    /// Computes the angle clockwise from the positive z-axis of the projection of this vector onto the
    /// x-z plane (in radians).
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// let u = Vector::principal_axes(1.0, std::f64::consts::FRAC_PI_6, 1.0);
    /// assert!((u.yaw() - std::f64::consts::FRAC_PI_6).abs() < 1e-10);
    /// ```
    pub fn yaw(&self) -> T {
        T::atan2(-self.x, self.z)
    }

    /// Computes the angle of this vector below the x-z plane (in radians).
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// let u = Vector::principal_axes(1.0, 1.0, std::f64::consts::FRAC_PI_3);
    /// assert!((u.pitch() - std::f64::consts::FRAC_PI_3).abs() < 1e-10);
    /// ```
    pub fn pitch(&self) -> T {
        (-self.y / self.len()).min(T::one()).max(-T::one()).asin()
    }

    /// Computes the squared length of this vector.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// let u = Vector::<f64>::rect(3.0, 4.0, 12.0);
    /// assert!((u.len_sq() - 169.0).abs() < 1e-10);
    /// ```
    pub fn len_sq(&self) -> T {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// Computes the length of this vector.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// let u = Vector::<f64>::rect(3.0, 4.0, 12.0);
    /// assert!((u.len() - 13.0).abs() < 1e-10);
    /// ```
    pub fn len(&self) -> T {
        self.len_sq().sqrt()
    }

    /// Normalizes this vector, meaning that its length will equal one but point in the same direction.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// use std::f64::consts::*;
    /// let mut u = Vector::rect(LN_2, -PI, E);
    /// u.normalize();
    /// assert!((u.len_sq() - 1.0).abs() < 1e-10);
    /// ```
    pub fn normalize(&mut self) {
        *self /= self.len();
    }

    /// Copies this vector, normalizes the copy, and returns that copy.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// use std::f64::consts::*;
    /// let u = Vector::<f64>::rect(LN_2, -PI, E);
    /// assert!((u.normalized().len_sq() - 1.0).abs() < 1e-10);
    /// ```
    pub fn normalized(self) -> Self {
        let mut copy = self;
        copy.normalize();
        copy
    }

    /// Computes the dot product between this vector and the given vector.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// let u = Vector::<f64>::rect(1.0, 2.0, 3.0);
    /// let v = Vector::<f64>::rect(-2.0, 4.0, -6.0);
    /// assert!((u.dot(&v) + 12.0).abs() < 1e-10);
    /// ```
    pub fn dot(&self, other: &Self) -> T {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Computes the cross product between this vector and the given vector.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// let mut u = Vector::<f64>::rect(1.0, 2.0, 3.0);
    /// let mut v = Vector::<f64>::rect(-2.0, 4.0, -6.0);
    ///
    /// let w = u.cross(&v);
    ///
    /// assert!(w.dot(&u).abs() < 1e-10);
    /// assert!(w.dot(&v).abs() < 1e-10);
    /// assert!((w.len_sq() - 640.0).abs() < 1e-10);
    /// ```
    pub fn cross(&self, other: &Self) -> Self {
        Vector {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x
        }
    }

    /// Rotates this vector about the x-axis according to the right-hand rule.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// use std::f64::consts::{FRAC_PI_4, SQRT_2};
    /// let mut u = Vector::rect(1.0, 2.0, 0.0);
    /// u.rotate_x(FRAC_PI_4);
    /// assert!((u - Vector::rect(1.0, SQRT_2, SQRT_2)).len_sq() < 1e-10);
    /// ```
    pub fn rotate_x(&mut self, angle: T) {
        let cos = angle.cos();
        let sin = angle.sin();

        let y = self.y;
        let z = self.z;
        self.y = y * cos - z * sin;
        self.z = y * sin + z * cos;
    }

    /// Rotates this vector about the y-axis according to the right-hand rule.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// use std::f64::consts::{FRAC_PI_4, SQRT_2};
    /// let mut u = Vector::rect(1.0, 2.0, 0.0);
    /// u.rotate_y(FRAC_PI_4);
    /// assert!((u - Vector::rect(1.0 / SQRT_2, 2.0, -1.0 / SQRT_2)).len_sq() < 1e-10);
    /// ```
    pub fn rotate_y(&mut self, angle: T) {
        let cos = angle.cos();
        let sin = angle.sin();

        let x = self.x;
        let z = self.z;
        self.x = x * cos + z * sin;
        self.z = -x * sin + z * cos;
    }

    /// Rotates this vector about the z-axis according to the right-hand rule.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// use std::f64::consts::{FRAC_PI_3, SQRT_2};
    /// let mut u = Vector::rect(3.0f64.sqrt() / 2.0, 0.5, 0.0);
    /// u.rotate_z(FRAC_PI_3);
    /// assert!((u - Vector::rect(0.0, 1.0, 0.0)).len_sq() < 1e-10);
    /// ```
    pub fn rotate_z(&mut self, angle: T) {
        let cos = angle.cos();
        let sin = angle.sin();

        let x = self.x;
        let y = self.y;
        self.x = x * cos - y * sin;
        self.y = x * sin + y * cos;
    }

    /// Rotates this vector about a generic axis according to the right-hand rule. Note that the axis
    /// must be normalized.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Vector;
    /// let mut u = Vector::rect(0.0, 1.0, 0.0);
    /// let axis = Vector::rect(1.0, 1.0, 1.0).normalized();
    /// u.rotate_about(&axis, 2.0 * std::f64::consts::FRAC_PI_3);
    /// assert!((u - Vector::rect(0.0, 0.0, 1.0)).len_sq() < 1e-10);
    /// ```
    pub fn rotate_about(&mut self, axis: &Self, angle: T) {
        let x = self.x;
        let y = self.y;
        let z = self.z;
        let ux = axis.x;
        let uy = axis.y;
        let uz = axis.z;
        let cos = angle.cos();
        let cos1m = T::one() - cos;
        let sin = angle.sin();
        let dot = self.dot(axis);
        self.x = ux * dot * cos1m + x * cos + (-uz * y + uy * z) * sin;
        self.y = uy * dot * cos1m + y * cos + (uz * x - ux * z) * sin;
        self.z = uz * dot * cos1m + z * cos + (-uy * x + ux * y) * sin;
    }
}

impl<T: Display> Display for Vector<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "<")?;
        self.x.fmt(f)?;
        write!(f, ", ")?;
        self.y.fmt(f)?;
        write!(f, ", ")?;
        self.z.fmt(f)?;
        write!(f, ">")
    }
}

impl<T: Add<Output = T> + Copy> Add for Vector<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Vector {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z
        }
    }
}

impl<T: Add<Output = T> + Copy> AddAssign for Vector<T> {
    fn add_assign(&mut self, rhs: Self) {
        self.x = self.x + rhs.x;
        self.y = self.y + rhs.y;
        self.z = self.z + rhs.z;
    }
}

impl<T: Sub<Output = T> + Copy> Sub for Vector<T> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Vector {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z
        }
    }
}

impl<T: Sub<Output = T> + Copy> SubAssign for Vector<T> {
    fn sub_assign(&mut self, rhs: Self) {
        self.x = self.x - rhs.x;
        self.y = self.y - rhs.y;
        self.z = self.z - rhs.z;
    }
}

impl<T: Neg<Output = T>> Neg for Vector<T> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Vector {
            x: -self.x,
            y: -self.y,
            z: -self.z
        }
    }
}

impl<T: Mul<Output = T> + Copy> Mul<T> for Vector<T> {
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        Vector {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs
        }
    }
}

impl<T: Mul<Output = T> + Copy> MulAssign<T> for Vector<T> {
    fn mul_assign(&mut self, rhs: T) {
        self.x = self.x * rhs;
        self.y = self.y * rhs;
        self.z = self.z * rhs;
    }
}

impl<T: Div<Output = T> + Copy> Div<T> for Vector<T> {
    type Output = Self;

    fn div(self, rhs: T) -> Self::Output {
        Vector {
            x: self.x / rhs,
            y: self.y / rhs,
            z: self.z / rhs
        }
    }
}

impl<T: Div<Output = T> + Copy> DivAssign<T> for Vector<T> {
    fn div_assign(&mut self, rhs: T) {
        self.x = self.x / rhs;
        self.y = self.y / rhs;
        self.z = self.z / rhs;
    }
}