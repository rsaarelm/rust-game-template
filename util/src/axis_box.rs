#![allow(clippy::needless_range_loop)]

use std::{
    fmt::Debug,
    ops::{Add, AddAssign, Div, Neg, Sub, SubAssign},
};

use num_traits::{AsPrimitive, Euclid, FromPrimitive, One, Zero};
use rand::{distr::uniform::SampleUniform, prelude::Distribution};
use serde::{Deserialize, Serialize};

/// An integer box describes a region over a discrete cellular lattice.
///
/// Axis boxes are mapped to lattices via a basis vector that determines the
/// dimensions of a unit cell in the space of a box.
pub type IntegerBox<const N: usize> = AxisBox<i32, N>;

pub type Rect<T> = AxisBox<T, 2>;
pub type Cube<T> = AxisBox<T, 3>;

pub trait Element:
    Copy
    + Default
    + PartialOrd
    + Sub<Output = Self>
    + Neg<Output = Self>
    + Div<Output = Self>
    + Zero
    + One
    + Debug
{
}

impl<T> Element for T where
    T: Copy
        + Default
        + PartialOrd
        + Sub<Output = Self>
        + Neg<Output = Self>
        + Div<Output = Self>
        + Zero
        + One
        + Debug
{
}

/// Axis box, a Cartesian product of several ranges.
///
/// Equivalent to an axis-aligned bounding rectangle, bounding box etc.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct AxisBox<T, const N: usize> {
    pub p0: [T; N],
    pub p1: [T; N],
}

impl<T, const N: usize> Serialize for AxisBox<T, N>
where
    T: Serialize + Element,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // WAIT: (here and deserialize) Use fixed-size arrays (nicer for IDM) once we are allowed to write [T; N * 2] (generic_const_exprs)
        let mut elts = Vec::new();
        for &i in self.p0.iter() {
            elts.push(i);
        }
        for &i in self.p1.iter() {
            elts.push(i);
        }
        elts.serialize(serializer)
    }
}

impl<'de, T, const N: usize> Deserialize<'de> for AxisBox<T, N>
where
    T: Deserialize<'de> + Element,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let elts = <Vec<T>>::deserialize(deserializer)?;
        if elts.len() != 2 * N {
            return Err(serde::de::Error::custom("bad element count"));
        }
        Ok(AxisBox::new(
            std::array::from_fn(|i| elts[i]),
            std::array::from_fn(|i| elts[i + N]),
        ))
    }
}

impl<T, const N: usize> AxisBox<T, N> {
    /// Faster than `AxisBox::new`, but does not check that dimensions are
    /// positive.
    ///
    /// # Safety
    ///
    /// Caller must ensure `p1[i] >= p0[i]` for all i.
    pub unsafe fn new_unsafe(
        p0: impl Into<[T; N]>,
        p1: impl Into<[T; N]>,
    ) -> Self {
        AxisBox {
            p0: p0.into(),
            p1: p1.into(),
        }
    }
}

impl<T: Default, const N: usize> Default for AxisBox<T, N> {
    fn default() -> Self {
        AxisBox {
            p0: std::array::from_fn(|_| T::default()),
            p1: std::array::from_fn(|_| T::default()),
        }
    }
}

impl<T: Element> From<[T; 4]> for AxisBox<T, 2> {
    fn from([x1, y1, x2, y2]: [T; 4]) -> Self {
        AxisBox::new([x1, y1], [x2, y2])
    }
}

impl<T: Element, const N: usize> AxisBox<T, N> {
    /// Create a new axis box. If p1 has components that are smaller than
    /// p0's, the corresponding range is clamped to zero.
    pub fn new(p0: impl Into<[T; N]>, p1: impl Into<[T; N]>) -> Self {
        let (p0, p1) = (p0.into(), p1.into());

        AxisBox {
            p0,
            p1: std::array::from_fn(|i| pmax(p0[i], p1[i])),
        }
    }

    /// Create a volume 1 unit box with origin at `p0`.
    pub fn unit(p0: impl Into<[T; N]>) -> Self {
        let p0 = p0.into();
        Self::new(p0, std::array::from_fn(|i| p0[i] + T::one()))
    }

    /// Create a cell from a lattice point using the given basis.
    pub fn cell(basis: impl Into<[T; N]>, point: impl Into<[i32; N]>) -> Self
    where
        T: FromPrimitive,
    {
        let basis = basis.into();
        let point = point.into();
        let p0 = std::array::from_fn(|i| {
            T::from_i32(point[i]).expect("bad cast") * basis[i]
        });
        let p1 = std::array::from_fn(|i| p0[i] + basis[i]);
        AxisBox::new(p0, p1)
    }

    /// Create the lattice cell in the given basis containing the given point.
    pub fn cell_containing(
        basis: impl Into<[T; N]>,
        pos: impl Into<[T; N]>,
    ) -> Self
    where
        T: Euclid + AsPrimitive<i32> + FromPrimitive,
    {
        let basis = basis.into();
        let pos = pos.into();
        let pos = std::array::from_fn(|i| (pos[i].div_euclid(&basis[i])).as_());
        Self::cell(basis, pos)
    }

    /// Get the lattice point for this box if it were a lattice cell.
    ///
    /// If the box is part of a lattice that has a box corner snapping to
    /// origin, this will be a dual of `cell`:
    ///
    /// ```
    /// use util::Rect;
    ///
    /// assert_eq!(Rect::new([120, 80], [130, 90]).lattice_point(), [12, 8]);
    /// assert_eq!(Rect::new([120, -80], [130, -70]).lattice_point(), [12, -8]);
    ///
    /// assert_eq!(Rect::cell([10, 10], [12, 8]), Rect::new([120, 80], [130, 90]));
    /// assert_eq!(Rect::cell([10, 10], [12, -8]), Rect::new([120, -80], [130, -70]));
    /// ```
    pub fn lattice_point(&self) -> [i32; N]
    where
        T: Euclid + AsPrimitive<i32>,
    {
        let dim = self.dim();
        std::array::from_fn(|i| self.p0[i].div_euclid(&dim[i]).as_())
    }

    /// Create a new axis box. If p1 has components that are smaller than
    /// p0's, the corresponding range is clamped to zero. Add 1 to the outer
    /// rim so point p1 will be included when iterating the box.
    pub fn new_inclusive(
        p0: impl Into<[T; N]>,
        p1: impl Into<[T; N]>,
    ) -> AxisBox<T, N> {
        let (p0, p1) = (p0.into(), p1.into());

        AxisBox {
            p0,
            p1: std::array::from_fn(|i| pmax(p0[i], p1[i]) + One::one()),
        }
    }

    pub fn sized(p: impl Into<[T; N]>) -> Self {
        AxisBox::new([T::zero(); N], p)
    }

    /// Builds an axis box from the elementwise minimum and maximum of the
    /// points in the input point cloud.
    ///
    /// NB. The resulting axis box does not contain the outer rim of the
    /// points since the component ranges are exclusive on the outer end.
    pub fn from_points(
        it: impl IntoIterator<Item = impl Into<[T; N]>>,
    ) -> AxisBox<T, N> {
        let mut it = it.into_iter();
        if let Some(p) = it.next().map(|e| e.into()) {
            let (p0, p1) =
                it.map(|e| e.into()).fold((p, p), |(mut p0, mut p1), p| {
                    for i in 0..N {
                        p0[i] = pmin(p0[i], p[i]);
                        p1[i] = pmax(p1[i], p[i]);
                    }
                    (p0, p1)
                });
            AxisBox { p0, p1 }
        } else {
            Default::default()
        }
    }

    /// Builds an axis box guaranteed to contain every point in the point
    /// cloud. For integer `T` the result is the smallest such axis box.
    pub fn from_points_inclusive(
        it: impl IntoIterator<Item = impl Into<[T; N]>>,
    ) -> AxisBox<T, N> {
        let mut it = it.into_iter();
        if let Some(p0) = it.next().map(|e| e.into()) {
            let mut p1 = p0;
            for e in p1.iter_mut() {
                *e = *e + T::one();
            }

            let (p0, p1) =
                it.map(|e| e.into()).fold((p0, p1), |(mut p0, mut p1), p| {
                    for i in 0..N {
                        p0[i] = pmin(p0[i], p[i]);
                        p1[i] = pmax(p1[i], p[i] + T::one());
                    }
                    (p0, p1)
                });
            AxisBox { p0, p1 }
        } else {
            Default::default()
        }
    }

    pub fn cast<U: Element + 'static>(&self) -> AxisBox<U, N>
    where
        T: Copy + AsPrimitive<U>,
    {
        AxisBox::new(
            std::array::from_fn(|i| self.p0[i].as_()),
            std::array::from_fn(|i| self.p1[i].as_()),
        )
    }

    pub fn is_empty(&self) -> bool {
        (0..N).any(|i| self.p1[i] <= self.p0[i])
    }

    pub fn contains(&self, e: impl Into<[T; N]>) -> bool {
        let e = e.into();
        (0..N).all(move |i| (self.p0[i]..self.p1[i]).contains(&e[i]))
    }

    pub fn contains_other(&self, r: &Self) -> bool {
        (0..N).all(|i| (self.p0[i] <= r.p0[i] && self.p1[i] >= r.p1[i]))
    }

    pub fn intersects(&self, r: &Self) -> bool {
        (0..N).all(|i| (r.p0[i] < self.p1[i] && r.p1[i] > self.p0[i]))
    }

    /// Return the product of the components of the dimension vector of the
    /// axis box.
    ///
    /// NB. This can overflow easily with large multidimensional axis boxes.
    pub fn volume(&self) -> T {
        (0..N)
            .map(move |i| self.p1[i] - self.p0[i])
            .fold(T::one(), |a, b| a * b)
    }

    /// Return vector with dimensions of the axis box.
    pub fn dim(&self) -> [T; N] {
        let mut ret = self.p1;
        for i in 0..N {
            ret[i] = ret[i] - self.p0[i];
        }
        ret
    }

    pub fn min(&self) -> [T; N] {
        self.p0
    }

    pub fn max(&self) -> [T; N] {
        self.p1
    }

    pub fn width(&self) -> T {
        self.p1[0] - self.p0[0]
    }

    pub fn height(&self) -> T {
        debug_assert!(N >= 2);
        self.p1[1] - self.p0[1]
    }

    pub fn depth(&self) -> T {
        debug_assert!(N >= 3);
        self.p1[2] - self.p0[2]
    }

    /// Grow the axis box, with parametrization for every facet.
    ///
    /// The first argument specifies expansion amounts of the "lower" facets
    /// opposite to the coordinate axes. The second specifies expansion of the
    /// "upper" facets pointing in the same direction as the coordinate axes.
    pub fn grow<U: Into<[T; N]>>(
        &self,
        lower_amount: U,
        upper_amount: U,
    ) -> Self {
        let lower_amount = lower_amount.into();
        let upper_amount = upper_amount.into();
        let (mut p0, mut p1) = (self.p0, self.p1);
        for i in 0..N {
            p0[i] = p0[i] - lower_amount[i];
            p1[i] = p1[i] + upper_amount[i];
        }

        AxisBox::new(p0, p1)
    }

    /// Convenience method, `grow` with the signs flipped.
    pub fn shrink<U: Into<[T; N]>>(
        &self,
        lower_amount: U,
        upper_amount: U,
    ) -> Self {
        let lower_amount = lower_amount.into();
        let upper_amount = upper_amount.into();
        let (mut p0, mut p1) = (self.p0, self.p1);
        for i in 0..N {
            p0[i] = p0[i] + lower_amount[i];
            p1[i] = p1[i] - upper_amount[i];
        }

        AxisBox::new(p0, p1)
    }

    pub fn center(&self) -> [T; N] {
        let two = T::one() + T::one();
        let dim = self.dim();
        let mut ret = self.p0;
        for i in 0..N {
            ret[i] = ret[i] + dim[i] / two;
        }
        ret
    }

    /// Return the axis box of the intersection of `self` and `rhs`.
    pub fn intersection(&self, rhs: &Self) -> Self {
        AxisBox::new(
            std::array::from_fn(|i| pmax(self.p0[i], rhs.p0[i])),
            std::array::from_fn(|i| pmin(self.p1[i], rhs.p1[i])),
        )
    }

    /// Return the smallest axis box that contains `self` and `rhs`.
    pub fn union(&self, rhs: &Self) -> Self {
        AxisBox::new(
            std::array::from_fn(|i| pmin(self.p0[i], rhs.p0[i])),
            std::array::from_fn(|i| pmax(self.p1[i], rhs.p1[i])),
        )
    }

    /// Projects a point into the inside of the axis box using modular
    /// arithmetic on each axis. A point leaving across one end will return on
    /// the other end.
    pub fn mod_proj<E>(&self, p: E) -> E
    where
        E: From<[T; N]> + Into<[T; N]>,
        T: Euclid,
    {
        let mut p = p.into();
        for i in 0..N {
            p[i] = p[i] - self.p0[i];
            if self.p1[i] != self.p0[i] {
                p[i] = p[i].rem_euclid(&(self.p1[i] - self.p0[i]));
            } else {
                p[i] = self.p1[i];
            }
            p[i] = p[i] + self.p0[i];
        }
        E::from(p)
    }

    /// Split the axis box along a plane specified by the vector.
    ///
    /// `split_plane` should have exactly one non-zero component along the
    /// axis which the axis box is being split. If it's positive, the split
    /// is made from the bottom face up by the magnitude of the component. If
    /// it's negative, the split is made from the top face down by the
    /// absolute magnitude of the component.
    pub fn split<E>(&self, split_plane: E) -> [Self; 2]
    where
        E: From<[T; N]> + Into<[T; N]>,
        T: Euclid,
    {
        let split_plane = split_plane.into();
        let s0 =
            self.mod_proj(std::array::from_fn(|i| self.p0[i] + split_plane[i]));
        let mut s1 =
            self.mod_proj(std::array::from_fn(|i| self.p1[i] + split_plane[i]));

        // Outer bounds can be outside domain of mod_proj, fix.
        for i in 0..N {
            if s1[i] == self.p0[i] {
                s1[i] = self.p1[i];
            }
        }

        [Self::new(self.p0, s1), Self::new(s0, self.p1)]
    }

    /// Clamp a vector type into the bounds of the box.
    pub fn clamp<E>(&self, val: E) -> E
    where
        E: From<[T; N]> + Into<[T; N]>,
    {
        let mut val = val.into();
        for i in 0..N {
            val[i] = pmax(self.p0[i], val[i]);
            val[i] = pmin(self.p1[i], val[i]);
        }

        E::from(val)
    }

    /// Return volume that contains origin positions for `smaller` box where
    /// `smaller` is fully contained in self.
    pub fn sweep_volume(&self, smaller: &Self) -> Self {
        let mut ret = *self;
        let self_size = self.dim();
        let size = smaller.dim();
        for i in 0..N {
            if size[i] <= self_size[i] {
                // Shrink the outer edge by dimensions of smaller.
                ret.p1[i] = ret.p1[i] - size[i];
            } else {
                // Bail out with a zero volume box if smaller turns out to not be
                // smaller along some dimension.
                return Default::default();
            }
        }
        ret
    }
}

impl<T: Element> AxisBox<T, 3> {
    pub fn flatten(&self) -> AxisBox<T, 2> {
        AxisBox::new(
            std::array::from_fn(|i| self.p0[i]),
            std::array::from_fn(|i| self.p1[i]),
        )
    }
}

impl<T, const N: usize> AxisBox<T, N>
where
    T: Element + Euclid + AsPrimitive<i32> + FromPrimitive,
{
    /// Return lattice box in given basis that has all cells that intersect
    /// with self.
    pub fn intersecting_lattice(
        &self,
        basis: impl Into<[T; N]>,
    ) -> IntegerBox<N> {
        let basis = basis.into();
        let p0 = std::array::from_fn(|i| div_floor(self.p0[i], basis[i]).as_());
        let p1 = std::array::from_fn(|i| div_ceil(self.p1[i], basis[i]).as_());
        IntegerBox::new(p0, p1)
    }

    /// Convenience method that iterates the lattice points as cell boxes.
    pub fn intersecting_lattice_iter(
        &self,
        basis: impl Into<[T; N]>,
    ) -> impl Iterator<Item = Self> {
        let basis = basis.into();
        self.intersecting_lattice(basis)
            .into_iter()
            .map(move |p| Self::cell(basis, p))
    }

    /// Return lattice box in given basis that has all cells that are fully
    /// contained in self.
    pub fn enclosed_lattice(&self, basis: impl Into<[T; N]>) -> IntegerBox<N> {
        let basis = basis.into();
        let p1 = std::array::from_fn(|i| div_floor(self.p1[i], basis[i]).as_());
        let p0 = std::array::from_fn(|i| {
            div_ceil(self.p0[i], basis[i]).as_().min(p1[i])
        });
        IntegerBox::new(p0, p1)
    }

    /// Convenience method that iterates the lattice points as cell boxes.
    pub fn enclosed_lattice_iter(
        &self,
        basis: impl Into<[T; N]>,
    ) -> impl Iterator<Item = Self> {
        let basis = basis.into();
        self.enclosed_lattice(basis)
            .into_iter()
            .map(move |p| Self::cell(basis, p))
    }
}

impl<T, const N: usize> AxisBox<T, N>
where
    T: Element + Euclid + AsPrimitive<f32> + FromPrimitive,
{
    pub fn split_frac(&self, split_plane: impl Into<[f32; N]>) -> [Self; 2] {
        let mut split_plane = split_plane.into();
        let dim = self.dim();
        for i in 0..N {
            split_plane[i] *= dim[i].as_();
        }

        self.split(std::array::from_fn(move |i| {
            let Some(x) = T::from_f32(split_plane[i]) else {
                panic!("casting failed");
            };
            x
        }))
    }

    pub fn grow_frac<U: Into<[f32; N]>>(
        &self,
        lower_amount: U,
        upper_amount: U,
    ) -> Self {
        let mut lower_amount = lower_amount.into();
        let mut upper_amount = upper_amount.into();

        let dim = self.dim();
        for i in 0..N {
            lower_amount[i] *= dim[i].as_();
            upper_amount[i] *= dim[i].as_();
        }

        self.grow(
            std::array::from_fn(move |i| {
                let Some(x) = T::from_f32(lower_amount[i]) else {
                    panic!("casting failed");
                };
                x
            }),
            std::array::from_fn(move |i| {
                let Some(x) = T::from_f32(upper_amount[i]) else {
                    panic!("casting failed");
                };
                x
            }),
        )
    }

    /// Return copy of self snapped to other so that relative anchor point (in
    /// range [0.0, 1.0] for any size) in other and the new axis box line up.
    pub fn snap_to(&self, other: &Self, anchor: impl Into<[f32; N]>) -> Self {
        let anchor = anchor.into();

        let d1 = self.dim();
        let d2 = other.dim();

        let mut p0 = other.p0;
        let mut p1 = d1;

        for i in 0..N {
            let f = anchor[i] * d2[i].as_() - anchor[i] * d1[i].as_();
            let Some(x) = T::from_f32(f) else {
                panic!("casting failed");
            };
            p0[i] = p0[i] + x;
            p1[i] = p1[i] + p0[i];
        }

        Self::new(p0, p1)
    }
}

impl<const N: usize> IntegerBox<N> {
    /// Get a lattice index for a point in the original space using the given
    /// lattice basis.
    pub fn idx_using<T>(
        &self,
        basis: impl Into<[T; N]>,
        p: impl Into<[T; N]>,
    ) -> usize
    where
        T: Element + Euclid + AsPrimitive<i32>,
    {
        let basis = basis.into();
        let p = p.into();
        let p = std::array::from_fn(|i| (p[i].div_euclid(&basis[i])).as_());
        self.idx(p)
    }

    /// Get a lattice index for a lattice point within the box. Points outside
    /// the box are wrapped into the box using a modulus operator for each
    /// coordinate.
    pub fn idx(&self, p: impl Into<[i32; N]>) -> usize {
        let p = p.into();

        let size: [i32; N] = self.dim();
        let mut span = [0; N];
        for i in 0..N {
            span[i] = size[i] as usize;
        }

        let mut ret = 0;
        let mut scale = 1;
        for i in 0..N {
            let x = (p[i] - self.p0[i]).rem_euclid(size[i]) as usize;
            ret += x * scale;
            scale *= span[i];
        }

        ret
    }

    /// Get the lattice box point for the given lattice index.
    pub fn get(&self, n: usize) -> [i32; N] {
        let size: [i32; N] = self.dim();
        let mut span = [0; N];
        for i in 0..N {
            let Ok(x) = size[i].try_into() else {
                panic!("bad range");
            };
            span[i] = x;
        }

        let mut v = [0; N];
        let mut scale = 1;
        for i in 0..N {
            v[i] = (n / scale) % span[i];
            scale *= span[i];
        }

        let mut e = [Default::default(); N];
        for i in 0..N {
            let Ok(x) = i32::try_from(v[i]) else {
                panic!("bad range");
            };
            e[i] = self.p0[i] + x;
        }
        e
    }

    /// Lattice cell count of lattice box.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.volume() as usize
    }

    /// Grow to contain given point.
    pub fn grow_to_contain(&self, p: impl Into<[i32; N]>) -> Self {
        let p = p.into();

        let mut ret = *self;
        for i in 0..N {
            ret.p0[i] = pmin(ret.p0[i], p[i]);
            ret.p1[i] = pmax(ret.p1[i], p[i] + 1);
        }

        ret
    }

    /// Return a sub-volume where for each element in normal, -1 will select
    /// the unit thickness minimum contained volume at the small end of that
    /// axis, 1 will select the unit thickness volume at the large end of that
    /// axis, and 0 will encompass the whole volume. Technically the border is
    /// a k-face where k is the axis box's `N` minus the number of nonzero
    /// components in `normal`.
    ///
    /// So for a cube, [0, 0, 0] will cover the whole cube, while [0, 0, -1]
    /// will produce the bottom face and [-1, 0, -1] will produce the bottom
    /// left edge.
    pub fn border(&self, normal: impl Into<[i32; N]>) -> Self {
        let normal = normal.into();
        let mut ret = *self;

        for i in 0..N {
            match normal[i] {
                n if n < 0 => ret.p1[i] = pmin(ret.p1[i], ret.p0[i] + 1),
                n if n > 0 => ret.p0[i] = pmax(ret.p0[i], ret.p1[i] - 1),
                _ => {}
            }
        }

        ret
    }

    /// Vector from closest contained integer point to given position.
    pub fn vec_to<E>(&self, p: E) -> E
    where
        E: From<[i32; N]> + Into<[i32; N]> + Sub<E, Output = E> + Clone,
    {
        p.clone() - self.clamp_inclusive(p)
    }

    /// Clamp a vector type into the box so that `contains` will be true for
    /// the result.
    pub fn clamp_inclusive<E>(&self, val: E) -> E
    where
        E: From<[i32; N]> + Into<[i32; N]>,
    {
        let mut val = val.into();
        for i in 0..N {
            val[i] = pmax(self.p0[i], val[i]);
            val[i] = pmin(self.p1[i] - 1, val[i]);
        }

        E::from(val)
    }

    /// Turn a lattice cell coordinate box into a regular space box consisting
    /// of the cells of the lattice.
    pub fn to_cells<T>(&self, basis: impl Into<[T; N]>) -> AxisBox<T, N>
    where
        T: FromPrimitive + Element,
    {
        let basis = basis.into();
        let mut max = self.max();
        for i in 0..N {
            max[i] -= 1;
        }

        AxisBox::cell(basis, self.min()).union(&AxisBox::cell(basis, max))
    }
}

impl IntegerBox<2> {
    /// Iterate through the outermost points in the rectangle.
    pub fn edge(&self) -> impl Iterator<Item = [i32; 2]> {
        let [x0, y0] = self.p0;
        let [x1, y1] = self.p1;
        let [x1, y1] = [x1 - 1, y1 - 1];

        (x0..x1).map(move |x| [x, y0]).chain(
            (y0..y1).map(move |y| [x1, y]).chain(
                ((x0 + 1)..=x1)
                    .rev()
                    .map(move |x| [x, y1])
                    .chain(((y0 + 1)..=y1).rev().map(move |y| [x0, y])),
            ),
        )
    }
}

impl<E, T, const N: usize> Add<E> for AxisBox<T, N>
where
    E: Into<[T; N]>,
    T: Element,
{
    type Output = AxisBox<T, N>;

    fn add(mut self, rhs: E) -> Self::Output {
        self += rhs;
        self
    }
}

impl<E, T, const N: usize> AddAssign<E> for AxisBox<T, N>
where
    E: Into<[T; N]>,
    T: Element,
{
    fn add_assign(&mut self, rhs: E) {
        let rhs = rhs.into();
        for i in 0..N {
            self.p0[i] = self.p0[i] + rhs[i];
            self.p1[i] = self.p1[i] + rhs[i];
        }
    }
}

impl<E, T, const N: usize> Sub<E> for AxisBox<T, N>
where
    E: Into<[T; N]>,
    T: Element,
{
    type Output = AxisBox<T, N>;

    fn sub(mut self, rhs: E) -> Self::Output {
        self -= rhs;
        self
    }
}

impl<E, T, const N: usize> SubAssign<E> for AxisBox<T, N>
where
    E: Into<[T; N]>,
    T: Element,
{
    fn sub_assign(&mut self, rhs: E) {
        let rhs = rhs.into();
        for i in 0..N {
            self.p0[i] = self.p0[i] - rhs[i];
            self.p1[i] = self.p1[i] - rhs[i];
        }
    }
}

impl<const N: usize, T, U> Distribution<U> for AxisBox<T, N>
where
    T: Element + SampleUniform,
    U: From<[T; N]>,
{
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> U {
        U::from(std::array::from_fn(|i| {
            rng.random_range(self.p0[i]..self.p1[i])
        }))
    }
}

impl<const N: usize> IntoIterator for IntegerBox<N> {
    type Item = [i32; N];

    type IntoIter = LatticeIter<N>;

    fn into_iter(self) -> LatticeIter<N> {
        LatticeIter {
            inner: self,
            x: self.p0,
        }
    }
}

pub struct LatticeIter<const N: usize> {
    inner: IntegerBox<N>,
    x: [i32; N],
}

impl<const N: usize> Iterator for LatticeIter<N> {
    type Item = [i32; N];

    fn next(&mut self) -> Option<Self::Item> {
        for i in 0..(N - 1) {
            if self.x[i] >= self.inner.p1[i] {
                self.x[i] = self.inner.p0[i];

                // One of the dimensions is zero, exit early.
                if self.x[i] >= self.inner.p1[i] {
                    return None;
                }

                self.x[i + 1] += 1;
            }
        }
        if self.x[N - 1] >= self.inner.p1[N - 1] {
            // Out of content.
            return None;
        }
        let ret = self.x;
        self.x[0] += 1;
        Some(ret)
    }
}

/// Return the larger of the two numbers. If the numbers can't be ordered, try
/// to return the number that can be ordered with itself.
pub fn pmin<T: PartialOrd>(a: T, b: T) -> T {
    if a < b {
        a
    } else if b.partial_cmp(&b).is_some() {
        b
    } else {
        a
    }
}

/// Return the smaller of the two numbers. If the numbers can't be ordered,
/// try to return the number that can be ordered with itself.
pub fn pmax<T: PartialOrd>(a: T, b: T) -> T {
    if a > b {
        a
    } else if b.partial_cmp(&b).is_some() {
        b
    } else {
        a
    }
}

/// Generic version of div_floor, for both ints and floats.
fn div_floor<T: Euclid>(lhs: T, rhs: T) -> T {
    lhs.div_euclid(&rhs)
}

/// Generic version of div_ceil, for both ints and floats.
fn div_ceil<T: Euclid + std::ops::Add<T> + PartialEq + Zero + One>(
    lhs: T,
    rhs: T,
) -> T {
    let mut ret = lhs.div_euclid(&rhs);
    if lhs.rem_euclid(&rhs) != T::zero() {
        ret = ret + T::one();
    }
    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexing() {
        let bounds: AxisBox<i32, 3> = AxisBox::new([1, 2, 3], [4, 5, 6]);

        for (i, p) in bounds.into_iter().enumerate() {
            if i == 0 {
                assert_eq!(p, [1, 2, 3]);
            }
            assert_eq!(i, bounds.idx(p));
            assert_eq!(bounds.get(i), p);
        }
    }

    #[test]
    fn pmin_pmax() {
        assert_eq!(pmax(1.0, 2.0), 2.0);
        assert_eq!(pmax(f32::NAN, 2.0), 2.0);
        assert_eq!(pmax(1.0, f32::NAN), 1.0);
        assert!(pmax(f32::NAN, f32::NAN).is_nan());

        assert_eq!(pmin(1.0, 2.0), 1.0);
        assert_eq!(pmin(f32::NAN, 2.0), 2.0);
        assert_eq!(pmin(1.0, f32::NAN), 1.0);
        assert!(pmin(f32::NAN, f32::NAN).is_nan());
    }

    #[test]
    fn custom_numeric_type() {
        type F = fraction::Fraction;
        let bounds = Rect::sized([F::from(10), F::from(20)]);

        assert_eq!(bounds.center(), [F::from(5), F::from(10)]);
    }

    #[test]
    fn intersects() {
        assert!(
            AxisBox::new([2, 2], [8, 8])
                .intersects(&AxisBox::new([4, 4], [6, 6]))
        );
        assert!(
            AxisBox::new([2, 2], [8, 8])
                .intersects(&AxisBox::new([0, 0], [10, 10]))
        );
        assert!(
            AxisBox::new([2, 2], [8, 8])
                .intersects(&AxisBox::new([5, 4], [10, 6]))
        );
        assert!(
            !AxisBox::new([2, 2], [8, 8])
                .intersects(&AxisBox::new([14, 4], [16, 6]))
        );
        assert!(
            !AxisBox::new([2, 2], [8, 8])
                .intersects(&AxisBox::new([4, 14], [6, 16]))
        );
    }

    #[test]
    fn split() {
        assert_eq!(
            Rect::sized([10, 10]).split([2, 0]),
            [Rect::new([0, 0], [2, 10]), Rect::new([2, 0], [10, 10])]
        );

        assert_eq!(
            Rect::sized([10, 10]).split([0, 2]),
            [Rect::new([0, 0], [10, 2]), Rect::new([0, 2], [10, 10])]
        );

        assert_eq!(
            Rect::sized([10, 10]).split([-2, 0]),
            [Rect::new([0, 0], [8, 10]), Rect::new([8, 0], [10, 10])]
        );

        assert_eq!(
            Rect::sized([10, 10]).split_frac([0.5, 0.0]),
            [Rect::new([0, 0], [5, 10]), Rect::new([5, 0], [10, 10])]
        );

        let cube: AxisBox<i32, 3> = AxisBox::sized([3, 4, 5]);
        for vec in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 2, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -2],
        ] {
            let [a, b] = cube.split(vec);
            eprintln!("{a:?} {b:?}");
            assert!(a.volume() > 0);
            assert!(b.volume() > 0);
            assert_eq!(a.union(&b), cube);
            assert!(a.intersection(&b).is_empty());
            assert!(cube.contains_other(&a));
            assert!(cube.contains_other(&b));
        }
    }

    #[test]
    fn iter() {
        assert_eq!(Rect::new([0, 0], [5, 5]).into_iter().count(), 25);
        assert_eq!(Rect::new([0, 0], [5, 0]).into_iter().count(), 0);
        assert_eq!(Rect::new([0, 0], [0, 5]).into_iter().count(), 0);
    }

    #[test]
    fn lattice_iter() {
        assert_eq!(
            Rect::new([2, 2], [4, 4])
                .intersecting_lattice([10, 10])
                .into_iter()
                .collect::<Vec<_>>(),
            vec![[0, 0]]
        );
        assert!(
            Rect::new([2, 2], [4, 4])
                .enclosed_lattice([10, 10])
                .into_iter()
                .collect::<Vec<_>>()
                .is_empty()
        );
        assert_eq!(
            Rect::new([2, 2], [14, 4])
                .intersecting_lattice([10, 10])
                .into_iter()
                .collect::<Vec<_>>(),
            vec![[0, 0], [1, 0]]
        );
        assert_eq!(
            Rect::new([0, 0], [10, 10])
                .enclosed_lattice([10, 10])
                .into_iter()
                .collect::<Vec<_>>(),
            vec![[0, 0]]
        );
        assert_eq!(
            Rect::new([-2, -2], [25, 12])
                .enclosed_lattice([10, 10])
                .into_iter()
                .collect::<Vec<_>>(),
            vec![[0, 0], [1, 0]]
        );
    }

    #[test]
    fn snap() {
        assert_eq!(
            Rect::new([0, 0], [10, 10])
                .snap_to(&Rect::new([20, 20], [80, 80]), [0.0, 0.0]),
            Rect::new([20, 20], [30, 30])
        );

        assert_eq!(
            Rect::new([0, 0], [10, 10])
                .snap_to(&Rect::new([20, 20], [80, 80]), [0.5, 0.5]),
            Rect::new([45, 45], [55, 55])
        );

        assert_eq!(
            Rect::new([0, 0], [10, 10])
                .snap_to(&Rect::new([20, 20], [80, 80]), [1.0, 0.0]),
            Rect::new([70, 20], [80, 30])
        );

        assert_eq!(
            Rect::new([0, 0], [10, 10])
                .snap_to(&Rect::new([20, 20], [80, 80]), [1.0, 1.0]),
            Rect::new([70, 70], [80, 80])
        );
    }
}
