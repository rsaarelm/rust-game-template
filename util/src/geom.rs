use std::{
    f32::consts::{PI, TAU},
    fmt,
    str::FromStr,
};

use glam::{ivec2, vec2, IVec2, IVec3, Vec2};
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// 8 directions, clock face order.
pub const DIR_8: [IVec2; 8] = [
    IVec2::from_array([0, -1]),
    IVec2::from_array([1, -1]),
    IVec2::from_array([1, 0]),
    IVec2::from_array([1, 1]),
    IVec2::from_array([0, 1]),
    IVec2::from_array([-1, 1]),
    IVec2::from_array([-1, 0]),
    IVec2::from_array([-1, -1]),
];

/// 4 directions, clock face order.
pub const DIR_4: [IVec2; 4] = [
    IVec2::from_array([0, -1]),
    IVec2::from_array([1, 0]),
    IVec2::from_array([0, 1]),
    IVec2::from_array([-1, 0]),
];

pub trait VecExt: Sized + Default {
    /// Absolute size of vector in taxicab metric.
    fn taxi_len(&self) -> i32;

    /// Vec points to an adjacent cell, left, right, up or down.
    fn is_adjacent(&self) -> bool {
        self.taxi_len() == 1
    }

    /// Tiebreaker method: Whether this position prefers horizontal 4-dirs.
    fn prefer_horizontals_here(&self) -> bool;

    /// Preferred cardinal direction vector pointing towards the other point.
    fn dir4_towards(&self, other: &Self) -> Self;

    fn to_dir4(&self) -> Self {
        Self::default().dir4_towards(self)
    }

    fn to_char(&self) -> char;

    fn is_wide_cell_center(&self) -> bool;
}

impl VecExt for IVec2 {
    fn taxi_len(&self) -> i32 {
        self[0].abs() + self[1].abs()
    }

    fn prefer_horizontals_here(&self) -> bool {
        // Whether we're starting from "white chessboard square" or "black
        // chessboard square". Tiebreaker preference for vertical or
        // horizontal move will alternate according to chessboard square color
        // so that repeating single steps of trying to move diagonally will
        // actually produce a diagonal path.
        (self[0] + self[1]).rem_euclid(2) == 0
    }

    fn dir4_towards(&self, other: &Self) -> Self {
        let (dx, dy) = (other[0] - self[0], other[1] - self[1]);
        let (adx, ady) = (dx.abs(), dy.abs());

        #[allow(clippy::if_same_then_else)]
        if ady > adx {
            ivec2(0, dy.signum())
        } else if adx > ady {
            ivec2(dx.signum(), 0)
        } else if self.prefer_horizontals_here() {
            // Absolute values are equal, use alternating tiebreaker to choose
            // between horizontal and vertical step.
            ivec2(dx.signum(), 0)
        } else {
            ivec2(0, dy.signum())
        }
    }

    fn to_char(&self) -> char {
        match <[i32; 2]>::from(IVec2::default().dir4_towards(self)) {
            [-1, 0] => '-',
            [1, 0] => '-',
            [0, -1] => '|',
            [0, 1] => '|',
            _ => '∙',
        }
    }

    fn is_wide_cell_center(&self) -> bool {
        self.x % 2 == 0
    }
}

impl VecExt for IVec3 {
    fn taxi_len(&self) -> i32 {
        self[0].abs() + self[1].abs() + self[2].abs()
    }

    fn prefer_horizontals_here(&self) -> bool {
        self.truncate().prefer_horizontals_here()
    }

    fn dir4_towards(&self, other: &Self) -> Self {
        self.truncate()
            .dir4_towards(&other.truncate())
            .extend(self.z)
    }

    fn to_char(&self) -> char {
        self.truncate().to_char()
    }

    fn is_wide_cell_center(&self) -> bool {
        self.truncate().is_wide_cell_center()
    }
}

pub fn bresenham_line(
    a: impl Into<IVec2>,
    b: impl Into<IVec2>,
) -> impl Iterator<Item = IVec2> {
    let (a, b): (IVec2, IVec2) = (a.into(), b.into());

    let d = b - a;
    let step = d.signum();
    let d = d.abs() * ivec2(1, -1);
    let mut p = a;
    let mut err = d.x + d.y;

    std::iter::from_fn(move || {
        if p == b {
            None
        } else {
            let ret = p;

            let e2 = 2 * err;
            if e2 >= d.y {
                err += d.y;
                p.x += step.x;
            }
            if e2 <= d.x {
                err += d.x;
                p.y += step.y;
            }
            Some(ret)
        }
    })
}

/// Floating-point valued point that plots a nice-looking line when repeatedly
/// sampled into a low-resolution pixel grid.
///
/// ```
/// # use std::collections::HashSet;
/// # use glam::{ivec2, IVec2, vec2};
/// # use util::PlottedPoint;
///
/// let mut pt = PlottedPoint::from(vec2(0.0, 0.0));
/// let d = vec2(0.23, 0.11);
///
/// let mut samples: HashSet<IVec2> = HashSet::default();
/// let mut bounds = ivec2(0, 0);
///
/// for _ in 0..52 {
///     let x = pt.sample();
///     bounds = bounds.max(x + ivec2(1, 1));
///     samples.insert(x);
///     *pt += d;
/// }
///
/// let mut s = String::new();
/// for y in 0..bounds.y {
///     for x in 0..bounds.x {
///         if samples.contains(&ivec2(x, y)) {
///             s.push('*');
///         } else {
///             s.push('.');
///         }
///     }
///     s.push('\n');
/// }
/// assert_eq!(s.trim(), "\
/// *...........
/// .**.........
/// ...***......
/// ......**....
/// ........**..
/// ..........**");
///
/// ```
#[derive(Copy, Clone, Default, Debug)]
pub struct PlottedPoint {
    inner: Vec2,
    grid_pos: Vec2,
}

impl From<Vec2> for PlottedPoint {
    fn from(p: Vec2) -> Self {
        PlottedPoint {
            inner: p,
            grid_pos: p.round(),
        }
    }
}

impl std::ops::Deref for PlottedPoint {
    type Target = Vec2;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for PlottedPoint {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl PlottedPoint {
    /// Get the pixel grid position of the point.
    pub fn sample(&mut self) -> IVec2 {
        // Problem: Naive sampling of cells causes points to move in an
        // awkward zig-zag pattern when moving in a diagonal-ish direction. To
        // solve this, delay updating the sampled position until we are
        // reasonably sure the point won't end up in a diagonal neighbor cell.
        //
        // Use radius of a circle going through side centers of neighboring
        // cells as the sampling threshold.
        //
        //    |     | :
        //    +-----+--x--+
        //    |     |   :
        //    |  o  |   :
        //    |     |   :
        //    +-----+--x--+
        //    |     | :
        //
        // sqrt(0.5^2 + 1^2) = sqrt(1.25)

        const UPDATE_DIST: f32 = 1.25;

        let d = self.inner - self.grid_pos;

        if d.dot(d) >= UPDATE_DIST {
            self.grid_pos = self.inner.round();
        }

        self.grid_pos.as_ivec2()
    }
}

/// Angle type, uses radians internally.
///
/// Angles use clock face convention, zero points at twelve o'clock, value
/// increases clockwise, rather than mathematical convention.
#[derive(
    Copy,
    Clone,
    PartialEq,
    PartialOrd,
    Default,
    Debug,
    DeserializeFromStr,
    SerializeDisplay,
)]
pub struct Angle(f32);

impl Angle {
    /// Initialize a new angle from a degree value.
    pub fn new(deg: f32) -> Self {
        Angle(deg * TAU / 360.0)
    }

    /// Snap the angle to its standard domain.
    pub fn normalize(self) -> Self {
        Angle((self.0 + PI).rem_euclid(TAU) - PI)
    }

    /// Absolute value of angle in degrees.
    pub fn abs(self) -> f32 {
        self.normalize().deg().abs()
    }

    /// Return the degree value of the angle.
    pub fn deg(self) -> f32 {
        self.0 * 360.0 / TAU
    }
}

impl From<Vec2> for Angle {
    fn from(value: Vec2) -> Self {
        Angle(value.x.atan2(-value.y))
    }
}

impl From<Angle> for Vec2 {
    fn from(value: Angle) -> Self {
        vec2(value.0.sin(), -value.0.cos())
    }
}

impl fmt::Display for Angle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.deg())
    }
}

impl FromStr for Angle {
    type Err = std::num::ParseFloatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Angle::new(s.parse()?))
    }
}

impl std::ops::Sub<Angle> for Angle {
    type Output = Angle;

    fn sub(self, rhs: Angle) -> Self::Output {
        Angle(self.0 - rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use super::*;
    use glam::vec2;

    #[test]
    fn angles() {
        assert_eq!(Angle::from(vec2(0.0, -1.0)).0, 0.0);
        assert_eq!(Angle::from(vec2(1.0, 0.0)).0, PI / 2.0);

        assert_eq!((Angle::new(10.0) - Angle::new(350.0)).abs().round(), 20.0);
        assert_eq!((Angle::new(370.0) - Angle::new(-10.0)).abs().round(), 20.0);
        assert_eq!((Angle::new(10.0) - Angle::new(30.0)).abs().round(), 20.0);
        assert_eq!((Angle::new(170.0) - Angle::new(190.0)).abs().round(), 20.0);

        assert_eq!(
            Vec2::from(Angle::from(vec2(0.0, -1.0))).round(),
            vec2(0.0, -1.0)
        );
        assert_eq!(
            Vec2::from(Angle::from(vec2(1.0, 0.0))).round(),
            vec2(1.0, 0.0)
        );
    }
}
