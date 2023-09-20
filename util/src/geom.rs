use std::{
    f32::consts::{PI, TAU},
    fmt,
    str::FromStr,
};

use glam::{ivec2, vec2, IVec2, IVec3, Vec2};
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// 4-directional grid space using taxicab metric.
pub mod s4 {
    use std::ops::Add;

    use glam::{ivec2, IVec2};

    use crate::VecExt;

    /// 4-dirs in clock face order.
    pub const DIR: [IVec2; 4] = [
        IVec2::from_array([0, -1]),
        IVec2::from_array([1, 0]),
        IVec2::from_array([0, 1]),
        IVec2::from_array([-1, 0]),
    ];

    /// Taxicab distance metric.
    pub fn d(a: &IVec2, b: &IVec2) -> i32 {
        let c = (*a - *b).abs();
        c.x + c.y
    }

    /// 4-neighbors of given point.
    pub fn ns<T: Clone + Add<IVec2, Output = T>>(
        p: T,
    ) -> impl Iterator<Item = T> {
        DIR.iter().map(move |d| p.clone() + *d)
    }

    /// Normalize vector to a 4-dir.
    pub fn norm(v: IVec2) -> IVec2 {
        norm_at(ivec2(0, 0), v)
    }

    /// Normalize vector from p1 to p2.
    ///
    /// Perfectly diagonal vectors will alternate between vertical and
    /// horizontal normalizations based on the starting point.
    pub fn norm_at(p1: IVec2, p2: IVec2) -> IVec2 {
        let (dx, dy) = (p2[0] - p1[0], p2[1] - p1[1]);
        let (adx, ady) = (dx.abs(), dy.abs());

        // if adx == ady alternate between horizontal and vertical dirs on
        // different starting positions to generate pseudo-diagonal movement.
        if ady > adx || (adx == ady && !p1.prefer_horizontals_here()) {
            if dy < 0 {
                DIR[0]
            } else {
                DIR[2]
            }
        } else if dx < 0 {
            DIR[3]
        } else {
            DIR[1]
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use glam::ivec2;

        #[test]
        fn dirs() {
            eprintln!("s4 test picture");
            for y in -5..=5 {
                for x in -5..=5 {
                    eprint!(
                        "{} ",
                        DIR.iter()
                            .position(|&a| a == norm(ivec2(x, y)))
                            .unwrap()
                    );
                }
                eprintln!()
            }

            for d in DIR {
                assert_eq!(norm(d), d);
            }
        }
    }
}

/// Hex coordinate space.
pub mod s6 {
    use std::{f32::consts::TAU, ops::Add};

    use glam::IVec2;

    /// 6-dirs.
    ///
    /// These are in clock face order when projected on screen to a flat-top
    /// hex display where the [-1, -1] axis points up and the [1, 0] axis
    /// points up and right.
    pub const DIR: [IVec2; 6] = [
        IVec2::from_array([-1, -1]),
        IVec2::from_array([0, -1]),
        IVec2::from_array([1, 0]),
        IVec2::from_array([1, 1]),
        IVec2::from_array([0, 1]),
        IVec2::from_array([-1, 0]),
    ];

    /// Hex distance metric.
    pub fn d(a: &IVec2, b: &IVec2) -> i32 {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        (dx.abs() + dy.abs() + (dx + dy).abs()) / 2
    }

    /// 6-neighbors of given point.
    pub fn ns<T: Clone + Add<IVec2, Output = T>>(
        p: T,
    ) -> impl Iterator<Item = T> {
        DIR.iter().map(move |d| p.clone() + *d)
    }

    /// Normalize a vector to a hex dir.
    ///
    /// ```notrust
    ///        *0*       *1*
    ///           \ 14 15 | 00 01
    ///           13\     |      02
    ///               \   |
    ///         12      \ |        03
    ///     *5* ----------O-X------- *2*
    ///         11        Y \      04
    ///                   |   \
    ///           10      |     \05
    ///             09 08 | 07 06 \
    ///                  *4*       *3*
    ///
    /// The hexadecants (00 to 15) and the hex
    /// directions (*0* to *5*) around the origin.
    /// ```
    ///
    /// Vectors that are in a space between two hex direction vectors are
    /// rounded to a hexadecant, then assigned the hex direction whose vector
    /// is nearest to that hexadecant.
    pub fn norm(v: IVec2) -> IVec2 {
        let hexadecant = {
            let width = TAU / 16.0;
            let mut radian = (v.x as f32).atan2(-v.y as f32);
            if radian < 0.0 {
                radian += TAU
            }
            (radian / width).floor() as i32
        };

        match hexadecant {
            13 | 14 => DIR[0],
            15 | 0 | 1 => DIR[1],
            2..=4 => DIR[2],
            5 | 6 => DIR[3],
            7..=9 => DIR[4],
            10..=12 => DIR[5],
            _ => panic!("Bad hexadecant"),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use glam::ivec2;

        #[test]
        fn dirs() {
            eprintln!("s6 test picture");
            for y in -5..=5 {
                for x in -5..=5 {
                    eprint!(
                        "{} ",
                        DIR.iter()
                            .position(|&a| a == norm(ivec2(x, y)))
                            .unwrap()
                    );
                }
                eprintln!()
            }

            for d in DIR {
                assert_eq!(norm(d), d);
            }
        }
    }
}

/// 8-directional grid space using chessboard metric.
pub mod s8 {
    use std::{f32::consts::TAU, ops::Add};

    use glam::IVec2;

    /// 8-dirs in clock face order.
    pub const DIR: [IVec2; 8] = [
        IVec2::from_array([0, -1]),
        IVec2::from_array([1, -1]),
        IVec2::from_array([1, 0]),
        IVec2::from_array([1, 1]),
        IVec2::from_array([0, 1]),
        IVec2::from_array([-1, 1]),
        IVec2::from_array([-1, 0]),
        IVec2::from_array([-1, -1]),
    ];

    /// Chessboard distance metric.
    pub fn d(a: &IVec2, b: &IVec2) -> i32 {
        let c = (*a - *b).abs();
        c.x.max(c.y)
    }

    /// 8-neighbors of given point.
    pub fn ns<T: Clone + Add<IVec2, Output = T>>(
        p: T,
    ) -> impl Iterator<Item = T> {
        DIR.iter().map(move |d| p.clone() + *d)
    }

    /// Normalize vector to a 8-dir.
    pub fn norm(v: IVec2) -> IVec2 {
        let a = ((v.x as f32).atan2(-v.y as f32) / TAU + 1.0 / 16.0)
            .rem_euclid(1.0)
            * 8.0;
        DIR[a as usize]
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use glam::ivec2;

        #[test]
        fn dirs() {
            eprintln!("s8 test picture");
            for y in -5..=5 {
                for x in -5..=5 {
                    eprint!(
                        "{} ",
                        DIR.iter()
                            .position(|&a| a == norm(ivec2(x, y)))
                            .unwrap()
                    );
                }
                eprintln!()
            }

            for d in DIR {
                assert_eq!(norm(d), d);
            }
        }
    }
}

/// Helper function for very concise IVec2 initialization.
pub fn v2(a: impl Into<glam::IVec2>) -> glam::IVec2 {
    a.into()
}

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
///     let x = pt.as_ivec2();
///     bounds = bounds.max(x + ivec2(1, 1));
///     samples.insert(x);
///     pt += d;
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
///
/// assert_eq!(s.trim(), "\
/// **..........
/// ..**........
/// ....**......
/// ......**....
/// ........**..
/// ..........**");
/// ```
#[derive(Copy, Clone, Default, Debug)]
pub struct PlottedPoint {
    inner: Vec2,
    delta: Vec2,
}

impl From<Vec2> for PlottedPoint {
    fn from(p: Vec2) -> Self {
        PlottedPoint {
            inner: p,
            delta: Default::default(),
        }
    }
}

impl std::ops::Deref for PlottedPoint {
    type Target = Vec2;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::AddAssign<Vec2> for PlottedPoint {
    fn add_assign(&mut self, delta: Vec2) {
        self.delta += delta;

        // Only change the actual position when the largest component of the
        // accumulated delta would change the rounded position component.

        // Index of largest accumulated delta component.
        let i = if self.delta[0] > self.delta[1] { 0 } else { 1 };

        if self.inner[i].round() != (self.inner[i] + self.delta[i]).round() {
            self.inner += self.delta;
            self.delta = Default::default();
        }
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
