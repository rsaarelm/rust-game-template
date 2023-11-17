//! Logic for revealing unexplored game terrain

use derive_more::{Deref, DerefMut};
use serde::{Deserialize, Serialize};

use crate::{prelude::*, BitAtlas};

/// Portions of map that have been revealed to player.
#[derive(Clone, Default, Deref, DerefMut, Serialize, Deserialize)]
#[serde(try_from = "BitAtlas", into = "BitAtlas")]
pub struct Fov(HashSet<Location>);

impl TryFrom<BitAtlas> for Fov {
    type Error = &'static str;

    fn try_from(value: BitAtlas) -> Result<Self, Self::Error> {
        let mut ret = Fov::default();

        for loc in value.iter() {
            ret.insert(loc);
        }
        Ok(ret)
    }
}

impl From<Fov> for BitAtlas {
    fn from(fov: Fov) -> Self {
        Self::from_iter(fov.0)
    }
}

impl Runtime {
    pub fn fov_from(
        &self,
        loc: Location,
        radius: i32,
    ) -> impl Iterator<Item = (IVec2, Location)> + '_ {
        #[derive(Copy, Clone)]
        struct FovState<'a> {
            origin: Location,
            r: &'a Runtime,
            radius: i32,
        }

        impl<'a> PartialEq for FovState<'a> {
            fn eq(&self, other: &Self) -> bool {
                self.origin == other.origin && self.radius == other.radius
            }
        }

        impl<'a> Eq for FovState<'a> {}

        impl<'a> FovState<'a> {
            pub fn new(
                origin: Location,
                r: &'a Runtime,
                radius: i32,
            ) -> FovState<'a> {
                FovState { origin, r, radius }
            }
        }

        impl<'a> fov::State for FovState<'a> {
            type Vector = glam::IVec2;

            fn advance(&self, offset: Self::Vector) -> Option<Self> {
                if offset.taxi_len() > self.radius {
                    return None;
                }

                if (self.origin + offset).is_wall_tile(self.r) {
                    return None;
                }

                Some(*self)
            }
        }

        fov::Square::new(FovState::new(loc, self, radius)).flat_map(|(v, s)| {
            (s.origin + v)
                .fov_volume(self)
                .into_iter()
                .map(move |loc| (v, loc))
        })
    }

    /// Return whether fog of war should be drawn at the given wide coordinate
    /// position.
    pub fn wide_pos_is_shrouded(&self, wide_loc_pos: IVec2) -> bool {
        let p = wide_loc_pos;
        if let Some(loc) = Location::fold_wide(p) {
            !loc.is_explored(self)
        } else {
            let c1 = Location::fold_wide(p - ivec2(1, 0)).unwrap();
            let c2 = Location::fold_wide(p + ivec2(1, 0)).unwrap();

            if c1.is_explored(self) && c2.is_explored(self) {
                return false;
            }

            // Fog sticks to itself and walls
            (!c1.is_explored(self) || c1.map_tile(self).is_wall())
                && (!c2.is_explored(self) || c2.map_tile(self).is_wall())
        }
    }
}
