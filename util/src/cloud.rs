use std::{collections::BTreeMap, fmt, str::FromStr};

use derive_more::Deref;

use crate::{text, IntegerBox};

#[derive(Clone, Eq, PartialEq, Debug, Deref)]
pub struct Cloud<const N: usize, V> {
    #[deref]
    points: BTreeMap<[i32; N], V>,
    bounds: IntegerBox<N>,
}

impl<V, const N: usize> Cloud<N, V> {
    pub fn insert(&mut self, p: impl Into<[i32; N]>, v: V) -> Option<V> {
        let p = p.into();
        if self.points.is_empty() {
            self.bounds = IntegerBox::from_points_inclusive(Some(p));
        } else {
            self.bounds = self.bounds.grow_to_contain(p);
        }
        self.points.insert(p, v)
    }

    /// Remove a point from the cloud.
    ///
    /// Does not shrink the bounding box, since this is an expensive
    /// operation. Call `recalculate_bounds` after removals if you need to
    /// tighten the bounds.
    pub fn remove(&mut self, p: impl Into<[i32; N]>) -> Option<V> {
        self.points.remove(&p.into())
    }

    pub fn bounds(&self) -> &IntegerBox<N> {
        &self.bounds
    }

    /// Recalculate the bounding box that might have become loose after
    /// removals.
    pub fn recalculate_bounds(&mut self) {
        self.bounds =
            IntegerBox::from_points_inclusive(self.points.keys().copied());
    }

    pub fn clear(&mut self) {
        self.points.clear();
        self.bounds = Default::default();
    }

    /// Iterate points where the positions have been normalized as with the
    /// `normalize` method.
    ///
    /// May not work correctly if `remove` has been called on clould and
    /// `recalculate_bounds` has not subsequently been called.
    pub fn normalized_iter(&self) -> impl Iterator<Item = ([i32; N], &'_ V)> {
        let min = self.bounds.min();
        self.points.iter().map(move |(p, v)| {
            let mut p = *p;
            for i in 0..N {
                p[i] -= min[i];
            }

            (p, v)
        })
    }

    /// Translate all points so that the minimum of every point coordinate is
    /// 0.
    pub fn normalize(&mut self) {
        self.recalculate_bounds();

        let min = self.bounds.min();
        let bounds = IntegerBox::sized(self.bounds.dim());

        // Rip out the old points and make a new value.
        let Cloud {
            points: old_points, ..
        } = std::mem::replace(
            self,
            Cloud {
                bounds,
                points: Default::default(),
            },
        );

        for (mut p, v) in old_points {
            for i in 0..N {
                p[i] -= min[i];
            }

            self.points.insert(p, v);
        }
    }
}

impl<V, const N: usize> Default for Cloud<N, V> {
    fn default() -> Self {
        Cloud {
            points: Default::default(),
            bounds: Default::default(),
        }
    }
}

impl<V, K: Into<[i32; N]>, const N: usize> Extend<(K, V)> for Cloud<N, V> {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (K, V)>,
    {
        for (p, v) in iter.into_iter() {
            self.insert(p, v);
        }
    }
}

impl<V, K: Into<[i32; N]>, const N: usize> FromIterator<(K, V)>
    for Cloud<N, V>
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut ret = Cloud::default();
        ret.extend(iter);
        ret
    }
}

impl<V, const N: usize> IntoIterator for Cloud<N, V> {
    type Item = ([i32; N], V);

    type IntoIter = std::collections::btree_map::IntoIter<[i32; N], V>;

    fn into_iter(self) -> Self::IntoIter {
        self.points.into_iter()
    }
}

impl FromStr for Cloud<2, char> {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(text::char_grid(s).collect())
    }
}

impl fmt::Display for Cloud<2, char> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut x = 0;
        let mut y = 0;
        let min = self.bounds().min();
        for p in self.bounds().into_iter() {
            let (px, py) = (p[0] - min[0], p[1] - min[1]);
            if let Some(c) = self.get(&p) {
                if c.is_whitespace() {
                    continue;
                }

                while py > y {
                    writeln!(f)?;
                    y += 1;
                    x = 0;
                }
                while px > x {
                    // Use NBSP so displayed clouds can be embedded in IDM.
                    write!(f, "\u{00a0}")?;
                    x += 1;
                }
                write!(f, "{c}")?;
                x += 1;
            }
        }

        Ok(())
    }
}
