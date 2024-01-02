use std::ops::{Add, AddAssign};

use glam::{ivec3, IVec2, IVec3};
use util::{s4, wallform_mask, Cloud, Neighbors2D};

use crate::{Block, Cube, Tile, Voxel, Zone, SECTOR_HEIGHT, SECTOR_WIDTH};

pub type Location = IVec3;

// Traits are used because we can't directly implement stuff for out-of-crate
// IVec3. There's no intention of using anything other than IVec3 for
// Location.

/// Methods for points when treated as game world locations.
pub trait Coordinates:
    Copy + Sized + Add<IVec3, Output = Self> + AddAssign<IVec3>
{
    fn z(&self) -> i32;

    fn voxel(&self, r: &impl Environs) -> Voxel;

    /// Snap location to origin of it's current 2D sector-slice.
    fn sector_snap_2d(&self) -> Self;

    /// Look for the valid neighboring floor adjacent to current location.
    ///
    /// Can step up or down one Z level. Returns `None` if terrain is blocked.
    fn walk_step(&self, r: &impl Environs, dir: IVec2) -> Option<Self> {
        let loc = *self + dir.extend(0);
        [loc.above(), loc, loc.below()]
            .into_iter()
            .find(|loc| loc.can_be_stood_in(r))
    }

    /// Like `walk_step`, but don't check whether there's solid support
    /// ground.
    ///
    /// Hover_step will move through doors.
    fn hover_step(&self, r: &impl Environs, dir: IVec2) -> Option<Self> {
        let loc = (*self + dir.extend(0)).snap_above_floor(r);
        match loc.voxel(r) {
            None | Some(Block::Door) => Some(loc),
            _ => None,
        }
    }

    fn walk_neighbors<'a>(
        self,
        r: &impl Environs,
    ) -> impl Iterator<Item = (IVec2, Self)> + '_;

    fn hover_neighbors<'a>(
        self,
        r: &impl Environs,
    ) -> impl Iterator<Item = (IVec2, Self)> + '_;

    /// Return the location directly above self.
    fn above(&self) -> Self {
        *self + ivec3(0, 0, 1)
    }

    /// Return the location directly below self.
    fn below(&self) -> Self {
        *self + ivec3(0, 0, -1)
    }

    /// Location is traversable space immediately above a support block.
    fn can_be_stood_in(&self, r: &impl Environs) -> bool {
        matches!(self.voxel(r), None | Some(Block::Door))
            && self.below().voxel(r).map_or(false, |b| b.is_support())
    }

    /// Return the pseudo-2D tile for terrain at given location.
    fn tile(&self, r: &impl Environs) -> Tile;

    /// Location is a wall tile and has wall tiles as all 8 neighbors.
    fn is_interior_wall(&self, r: &impl Environs) -> bool;

    /// If the location has a surface, snap to the space above the surface.
    /// This may be offset above or below self.
    ///
    /// Otherwise return self unchanged.
    fn snap_above_floor(&self, r: &impl Environs) -> Self;

    fn ground_voxel(&self, r: &impl Environs) -> Option<Location> {
        if let Tile::Surface(loc, _) = self.tile(r) {
            Some(loc.below())
        } else {
            None
        }
    }

    /// 4-bit mask that has 1 on direction with a step up.
    fn high_connectivity(&self, r: &impl Environs) -> usize {
        s4::DIR
            .iter()
            .enumerate()
            .map(|(i, &d)| {
                if self.walk_step(r, d).map_or(false, |loc| loc.z() > self.z())
                {
                    1 << i
                } else {
                    0
                }
            })
            .sum()
    }

    /// 4-bit mask that has 1 on direction with a step down.
    fn low_connectivity(&self, r: &impl Environs) -> usize {
        s4::DIR
            .iter()
            .enumerate()
            .map(|(i, &d)| {
                if self.walk_step(r, d).map_or(false, |loc| loc.z() < self.z())
                {
                    1 << i
                } else {
                    0
                }
            })
            .sum()
    }

    fn is_impassable(&self, r: &impl Environs) -> bool {
        match self.tile(r) {
            Tile::Void => true,
            Tile::Wall(b) if b != Block::Door => true,
            Tile::Surface(_, b) if !b.is_support() => true,
            _ => false,
        }
    }

    /// Return whether this location produces a z+1 floor and at least one
    /// 8-adjacent location produces a z-1 floor. Returns the mask of
    /// 4-adjacent cliff tiles.
    fn cliff_form(&self, r: &impl Environs) -> Option<usize>;

    /// For FoV calculations, volume of non-opaque tiles within the current
    /// sector slice.
    ///
    /// If the result is empty, this location should be treated as an opaque
    /// tile in terms of FoV.
    fn transparent_volume<'a>(&'a self, r: &'a impl Environs) -> Vec<Self> {
        let is_transparent =
            |loc: &Self| matches!(loc.voxel(r), None | Some(Block::Glass));

        let mut ret = Vec::new();
        if is_transparent(&self.above()) {
            ret.push(self.above());
        }
        if is_transparent(self) {
            ret.push(*self);
            if is_transparent(&self.below()) {
                ret.push(self.below());
            }
        }
        ret
    }

    /// Convenience method that doubles the x coordinate.
    ///
    /// Use for double-width character display.
    fn widen(&self) -> IVec3;

    /// Return the 1 z level thick sector zone of this location.
    fn sector(&self) -> Cube;

    fn at_sector_edge(&self) -> bool;

    /// Return a vector pointing to other location if locations are within 1 z
    /// level from each other.
    fn vec2_towards(&self, other: &Self) -> Option<IVec2>;

    /// Same sector plus the facing rims of adjacent sectors.
    fn has_same_screen_as(&self, other: &Self) -> bool;

    // Start tracing from self towards `dir` in `dir` size steps. Starts
    // from the point one step away from self. Panics if `dir` is a zero
    // vector. Does not follow portals.
    fn trace(&self, dir: IVec2) -> impl Iterator<Item = Self> {
        assert!(dir != IVec2::ZERO);

        let mut p = *self;
        std::iter::from_fn(move || {
            p += dir.extend(0);
            Some(p)
        })
    }
}

impl Coordinates for Location {
    fn z(&self) -> i32 {
        self.z
    }

    fn voxel(&self, r: &impl Environs) -> Voxel {
        r.voxel(self)
    }

    fn sector_snap_2d(&self) -> Self {
        ivec3(
            self.x.div_floor(SECTOR_WIDTH) * SECTOR_WIDTH,
            self.y.div_floor(SECTOR_HEIGHT) * SECTOR_HEIGHT,
            self.z,
        )
    }

    fn walk_neighbors<'a>(
        self,
        r: &impl Environs,
    ) -> impl Iterator<Item = (IVec2, Self)> + '_ {
        self.ns_4_alternating().filter_map(move |loc_2| {
            let d = (loc_2 - self).truncate();
            self.walk_step(r, d).map(|loc| (d, loc))
        })
    }

    fn hover_neighbors<'a>(
        self,
        r: &impl Environs,
    ) -> impl Iterator<Item = (IVec2, Self)> + '_ {
        self.ns_4_alternating().filter_map(move |loc_2| {
            let d = (loc_2 - self).truncate();
            self.hover_step(r, d).map(|loc| (d, loc))
        })
    }

    fn tile(&self, r: &impl Environs) -> Tile {
        use Block::*;

        match (self.above().voxel(r), self.voxel(r), self.below().voxel(r)) {
            // Solid three block stack, makes a proper wall.
            (Some(a), Some(b), c) => {
                // HACK Doors change traversability of the tile, so snap to
                // the door block even if it's found off-center.
                if a == Door || c == Some(Door) {
                    Tile::Wall(Door)
                } else {
                    Tile::Wall(b)
                }
            }
            // Raised floor.
            (None, Some(a), _) => Tile::Surface(*self + ivec3(0, 0, 1), a),
            // Regular floor
            (_, None, Some(a)) => Tile::Surface(*self, a),
            // Depressed floor, check further down if there's surface.
            (_, None, None) => {
                if let Some(a) = self.below().below().voxel(r) {
                    Tile::Surface(*self + ivec3(0, 0, -1), a)
                } else {
                    Tile::Void
                }
            }
        }
    }

    fn is_interior_wall(&self, r: &impl Environs) -> bool {
        self.tile(r).is_wall() && self.ns_8().all(|loc| loc.tile(r).is_wall())
    }

    fn snap_above_floor(&self, r: &impl Environs) -> Self {
        match self.tile(r) {
            Tile::Surface(loc, _) => loc,
            _ => *self,
        }
    }

    fn cliff_form(&self, r: &impl Environs) -> Option<usize> {
        fn is_mesa(loc: Location, r: &impl Environs) -> bool {
            matches!(loc.tile(r), Tile::Surface(b, _) if b.z > loc.z)
        }

        fn is_cliff(loc: Location, r: &impl Environs) -> bool {
            is_mesa(loc, r)
                && (loc + ivec3(0, 0, 1))
                    .ns_8()
                    .any(|a| matches!(a.tile(r), Tile::Void))
        }

        if is_cliff(*self, r) {
            let Some(mask) = wallform_mask(
                |loc| is_mesa(loc, r) || matches!(loc.tile(r), Tile::Wall(_)),
                *self,
            ) else {
                return None;
            };
            // Ignore cliff bits that aren't connected to any other cliff.
            // They seem to mostly end up being display noise.
            if mask != 0 {
                Some(mask)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn widen(&self) -> IVec3 {
        *self * ivec3(2, 1, 1)
    }

    fn sector(&self) -> Cube {
        Zone::sector_from(self)
    }

    fn at_sector_edge(&self) -> bool {
        let (u, v) = (
            self.x.rem_euclid(SECTOR_WIDTH),
            self.y.rem_euclid(SECTOR_HEIGHT),
        );
        (u == 0 || u == (SECTOR_WIDTH - 1))
            || (v == 0 || v == (SECTOR_HEIGHT - 1))
    }

    fn vec2_towards(&self, other: &Self) -> Option<IVec2> {
        if (self.z - other.z).abs() <= 1 {
            Some((*other - *self).truncate())
        } else {
            None
        }
    }

    fn has_same_screen_as(&self, other: &Self) -> bool {
        self.sector().fat().wide().contains(*other)
    }
}

pub trait Environs {
    fn voxel(&self, loc: &Location) -> Voxel;
    fn set_voxel(&mut self, loc: &Location, voxel: Voxel);
}

impl Environs for Cloud<3, Voxel> {
    fn voxel(&self, loc: &Location) -> Voxel {
        util::HashMap::get(self, &<[i32; 3]>::from(*loc))
            .copied()
            .unwrap_or_default()
    }

    fn set_voxel(&mut self, loc: &Location, voxel: Voxel) {
        // XXX: Empty voxels become explicit when first set and there isn't an
        // interface to forget about them in Environs.
        self.insert(*loc, voxel);
    }
}
