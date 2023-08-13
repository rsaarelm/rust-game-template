use crate::fov::Geometry;

/// Points on a square expressed in polar coordinates.
///
/// For each square with radius `r` (distance from origin cell to cell on
/// square perimeter along a coordinate axis), there are `8r` perimeter
/// points. The points are indexed starting from the bottom right (1, 1)
/// corner and going counterclockwise. The track is a floating-point valued
/// rectangular path along the centers of the perimeter cells.
///
/// ```notrust
/// |-----+-----+-----|
/// |     |     |     |
/// |     |     |     |
/// |     |     |     |
/// |-----+-----+-----|
/// |     |     |  ^  |
/// |     | 0,0 |  :  -  a = 1.0
/// |     |     |  :  |
/// |-----+-----+-----|
/// |     |     | \:  |
/// |     | >...|..\  -  a = 0.0
/// |     |     |   \ |
/// |-----+--|--+-----|
///
///        a = 7.0
/// ```
#[derive(Copy, Clone, PartialEq)]
pub struct SquareGeometry<V> {
    /// From 0.0 to 4.0 to encompass the square.
    pos: f32,
    /// How many cells away from origin we are.
    ///
    /// Perimeter is 8 * radius.
    radius: u32,

    phantom: std::marker::PhantomData<V>,
}

impl<V> SquareGeometry<V> {
    /// Index of the discrete hex cell along the circle that corresponds to
    /// this point.
    fn winding_index(self) -> i32 {
        (self.pos + 0.5).floor() as i32
    }

    fn end_index(self) -> i32 {
        (self.pos + 0.5).ceil() as i32
    }
}

impl<V: From<[i32; 2]> + Copy + Clone> Geometry for SquareGeometry<V> {
    type Vector = V;

    fn unit_circle_endpoints() -> (Self, Self) {
        (
            SquareGeometry {
                pos: 0.0,
                radius: 1,
                phantom: Default::default(),
            },
            SquareGeometry {
                pos: 8.0,
                radius: 1,
                phantom: Default::default(),
            },
        )
    }

    fn is_below(&self, other: &Self) -> bool {
        self.winding_index() < other.end_index()
    }

    fn to_v2(&self) -> Self::Vector {
        let index = self.winding_index();

        let r = self.radius as i32;
        let quadrant = index.rem_euclid(8 * r) / (2 * r);
        let a = index.rem_euclid(2 * r);

        match quadrant {
            0 => Self::Vector::from([r, r - a]),
            1 => Self::Vector::from([r - a, -r]),
            2 => Self::Vector::from([-r, a - r]),
            3 => Self::Vector::from([a - r, r]),
            _ => unreachable!(),
        }
    }

    fn expand(&self) -> Self {
        let r = self.radius as f32;
        SquareGeometry {
            pos: self.pos * (r + 1.0) / r,
            radius: self.radius + 1,
            phantom: Default::default(),
        }
    }

    fn advance(&mut self) {
        self.pos = (self.pos + 0.5).floor() + 0.5;
    }
}
