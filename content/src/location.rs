use std::ops::{Add, AddAssign};

use glam::{ivec2, ivec3, IVec2, IVec3};
use util::{s4, wallform_mask, Cloud, Neighbors2D};

use crate::{Block, Rect, Tile, Tile2D, Voxel, SECTOR_HEIGHT, SECTOR_WIDTH};

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

    /// 2D rectangle for the sector the location is in.
    fn sector_rect(&self) -> Rect;

    /// Look for the valid neighboring floor adjacent to current location.
    ///
    /// Can step up or down one Z level. Returns `None` if terrain is blocked.
    fn walk_step(&self, r: &impl Environs, dir: IVec2) -> Option<Self> {
        let loc = *self + dir.extend(0);
        [loc.above(), loc, loc.below()]
            .into_iter()
            .find(|loc| loc.can_be_stood_in(r))
    }

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

    /// Return whether this location produces a z+1 floor and at least one
    /// 8-adjacent location produces a z-1 floor. Returns the mask of
    /// 4-adjacent cliff tiles.
    fn cliff_form(&self, r: &impl Environs) -> Option<usize>;

    /// For FoV calculations, volume of non-opaque tiles within the current
    /// sector slice.
    ///
    /// If the result is empty, this location should be treated as an opaque
    /// tile in terms of FoV.
    fn transparent_volume<'a>(
        &'a self,
        r: &'a impl Environs,
    ) -> impl Iterator<Item = Self> + 'a {
        [self.above(), *self, self.below()]
            .into_iter()
            .filter(|loc| matches!(loc.voxel(r), None | Some(Block::Glass)))
    }

    // TODO: Deprecate fold methods
    /// Convert to 2D vector, layering Z-levels vertically in 2-plane.
    ///
    /// Each location has a unique point on the `IVec2` plane and the original
    /// location can be retrieved by calling `Coordinates::fold` on the
    /// `IVec2` value.
    fn unfold(&self) -> IVec2;

    /// Convert an unfolded 2D vector back to a Location.
    fn fold(loc_pos: impl Into<IVec2>) -> Self;

    /// Convenience method that doubles the x coordinate.
    ///
    /// Use for double-width character display.
    fn widen(&self) -> IVec3;

    fn fold_wide(wide_loc_pos: impl Into<IVec2>) -> Option<Self> {
        let wide_loc_pos = wide_loc_pos.into();

        if wide_loc_pos.x % 2 == 0 {
            Some(Coordinates::fold(wide_loc_pos / ivec2(2, 1)))
        } else {
            None
        }
    }

    /// Return the two locations on two sides of an off-center wide pos.
    ///
    /// If pos is not off-center, returns the same centered location twice.
    fn fold_wide_sides(wide_loc_pos: impl Into<IVec2>) -> (Self, Self) {
        let wide_loc_pos = wide_loc_pos.into();

        match Self::fold_wide(wide_loc_pos) {
            Some(loc) => (loc, loc),
            None => (
                Self::fold_wide(wide_loc_pos - ivec2(1, 0)).unwrap(),
                Self::fold_wide(wide_loc_pos + ivec2(1, 0)).unwrap(),
            ),
        }
    }

    /// Return location snapped to the origin of this location's sector.
    fn sector(&self) -> Location;

    /// How many sector transitions there are between self and other.
    fn sector_dist(&self, other: &Self) -> usize;

    /// Return sector bounding box containing this loc.
    fn sector_bounds(&self) -> Rect {
        let p = self.sector().unfold();
        Rect::new(p, p + ivec2(SECTOR_WIDTH, SECTOR_HEIGHT))
    }

    fn at_sector_edge(&self) -> bool;

    /// Return sector bounds extended for the adjacent sector rim.
    fn expanded_sector_bounds(&self) -> Rect {
        self.sector_bounds().grow([1, 1], [1, 1])
    }

    fn vec_towards(&self, other: &Self) -> Option<IVec2>;

    /// Same sector plus the facing rims of adjacent sectors.
    fn has_same_screen_as(&self, other: &Self) -> bool {
        self.expanded_sector_bounds().contains(other.unfold())
    }

    fn astar_heuristic(&self, other: &Self) -> usize;

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

    fn sector_rect(&self) -> Rect {
        let p = self.sector_snap_2d().truncate();
        Rect::new(p, p + ivec2(SECTOR_WIDTH, SECTOR_HEIGHT))
    }

    fn tile(&self, r: &impl Environs) -> Tile {
        use Block::*;

        match (self.above().voxel(r), self.voxel(r), self.below().voxel(r)) {
            // Solid three block stack, makes a proper wall.
            (Some(a), Some(b), Some(c)) => {
                // HACK Doors change traversability of the tile, so snap to
                // the door block even if it's found off-center.
                if a == Door || c == Door {
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
            (_, _, None) => {
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

        fn is_depression(loc: Location, r: &impl Environs) -> bool {
            matches!(loc.tile(r), Tile::Surface(b, _) if b.z < loc.z)
        }

        fn is_cliff(loc: Location, r: &impl Environs) -> bool {
            is_mesa(loc, r) && loc.ns_8().any(|a| is_depression(a, r))
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

    fn unfold(&self) -> IVec2 {
        // Maps y: i16::MIN, z: i16::MIN to i32::MIN.
        let y = self.y as i64 + self.z as i64 * 0x1_0000 - i16::MIN as i64;
        ivec2(self.x, y as i32)
    }

    fn fold(loc_pos: impl Into<IVec2>) -> Self {
        let loc_pos = loc_pos.into();

        let x = loc_pos.x;
        let y =
            ((loc_pos.y as i64).rem_euclid(0x1_0000) + i16::MIN as i64) as i32;
        let z = (loc_pos.y as i64).div_euclid(0x1_0000) as i32;

        ivec3(x, y, z)
    }

    fn widen(&self) -> IVec3 {
        *self * ivec3(2, 1, 1)
    }

    fn sector(&self) -> Location {
        Location::new(
            self.x.div_floor(SECTOR_WIDTH) * SECTOR_WIDTH,
            self.y.div_floor(SECTOR_HEIGHT) * SECTOR_HEIGHT,
            self.z,
        )
    }

    fn sector_dist(&self, other: &Self) -> usize {
        let a = Into::<IVec3>::into(self.sector())
            / ivec3(SECTOR_WIDTH, SECTOR_HEIGHT, 1);
        let b = Into::<IVec3>::into(other.sector())
            / ivec3(SECTOR_WIDTH, SECTOR_HEIGHT, 1);
        let d = (a - b).abs();
        (d.x + d.y + d.z) as usize
    }

    fn at_sector_edge(&self) -> bool {
        let (u, v) = (
            self.x.rem_euclid(SECTOR_WIDTH),
            self.y.rem_euclid(SECTOR_HEIGHT),
        );
        (u == 0 || u == (SECTOR_WIDTH - 1))
            || (v == 0 || v == (SECTOR_HEIGHT - 1))
    }

    fn vec_towards(&self, other: &Self) -> Option<IVec2> {
        if self.z == other.z {
            Some((*other - *self).truncate())
        } else {
            None
        }
    }

    fn astar_heuristic(&self, other: &Self) -> usize {
        // NB. This will work badly if pathing between Z-layers.
        let d = (*self - *other).abs();
        (d.x + d.y + d.z) as usize
    }
}

pub trait Environs {
    // TODO Deprecate tile interface after voxelization
    fn tile_2d(&self, loc: &Location) -> Tile2D {
        use Block::*;

        match (self.voxel(&loc.below()), self.voxel(loc)) {
            (Some(Rock), None) => Tile2D::Ground,
            (Some(Grass), None) => Tile2D::Grass,
            (Some(Water), None) => Tile2D::Water,
            (Some(Magma), None) => Tile2D::Magma,
            (_, Some(Rock)) => Tile2D::Wall,
            (_, Some(Door)) => Tile2D::Door,
            // Don't know, whatever.
            (Some(_), None) => Tile2D::Ground,
            (_, Some(_)) => Tile2D::Wall,
            // Chasms or stairs, not handled now...
            (None, None) => Tile2D::Ground,
        }
    }

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
