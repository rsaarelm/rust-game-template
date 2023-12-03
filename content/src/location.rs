use glam::{ivec2, ivec3, IVec2, IVec3};
use util::{s4, s8, Cloud};

use crate::{Rect, Tile, Tile2D, Voxel, SECTOR_HEIGHT, SECTOR_WIDTH};

pub type Location = IVec3;

/// Methods for points when treated as game world locations.
pub trait Coordinates: Copy + Sized {
    fn z(&self) -> i32;

    /// Snap location to origin of it's current 2D sector-slice.
    fn sector_snap_2d(&self) -> Self;

    /// If there is a tile floor location (an empty voxel above a filled
    /// voxel) that corresponds to the given location, snap to that location.
    fn snap_to_floor(&self, r: &impl Environs) -> Option<Self>;

    /// Look for the valid neighboring floor adjacent to current location.
    ///
    /// Can step up or down one Z level. Returns `None` if terrain is blocked.
    fn step(&self, r: &impl Environs, dir: IVec2) -> Option<Self>;

    /// Return the location directly above self.
    fn above(&self) -> Self;

    /// Return the location directly below self.
    fn below(&self) -> Self;

    /// Return whether location is solid in the environs and can be stood on.
    fn is_solid(&self, r: &impl Environs) -> bool;

    fn can_be_entered(&self, r: &impl Environs) -> bool;

    fn can_be_stepped_in(&self, r: &impl Environs) -> bool {
        self.can_be_entered(r) && self.below().is_solid(r)
    }

    /// Return the pseudo-2D tile for terrain at given location.
    fn tile(&self, r: &impl Environs) -> Tile;

    /// Convenience method that's fast to call.
    fn is_wall_tile(&self, r: &impl Environs) -> bool {
        self.above().is_solid(r) && self.is_solid(r)
    }

    /// 4-bit mask that has 1 on direction with a step up.
    fn high_connectivity(&self, r: &impl Environs) -> usize {
        s4::DIR
            .iter()
            .enumerate()
            .map(|(i, &d)| {
                if self.step(r, d).map_or(false, |loc| loc.z() > self.z()) {
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
                if self.step(r, d).map_or(false, |loc| loc.z() < self.z()) {
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
    fn unfold_wide(&self) -> IVec2 {
        let mut ret = self.unfold();
        ret.x *= 2;
        ret
    }

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
    fn trace(&self, dir: IVec2) -> impl Iterator<Item = Self>;
}

impl Coordinates for Location {
    fn z(&self) -> i32 {
        self.z
    }

    fn sector_snap_2d(&self) -> Self {
        ivec3(
            self.x.div_floor(SECTOR_WIDTH) * SECTOR_WIDTH,
            self.y.div_floor(SECTOR_HEIGHT) * SECTOR_HEIGHT,
            self.z,
        )
    }

    fn snap_to_floor(&self, r: &impl Environs) -> Option<Self> {
        match (
            self.above().is_solid(r),
            self.is_solid(r),
            self.below().is_solid(r),
            self.below().below().is_solid(r),
        ) {
            (false, true, _, _) => Some(self.above()),
            (_, false, true, _) => Some(*self),
            (_, _, false, true) => Some(self.below()),
            _ => None,
        }
    }

    /// Look for the valid neighboring floor adjacent to current location.
    ///
    /// Can step up or down one Z level. Returns `None` if terrain is blocked.
    fn step(&self, r: &impl Environs, dir: IVec2) -> Option<Self> {
        if let Some(loc) = (*self + dir.extend(0)).snap_to_floor(r) {
            if loc.can_be_stepped_in(r) {
                return Some(loc);
            }
        }

        None
    }

    fn above(&self) -> Self {
        *self + ivec3(0, 0, 1)
    }

    fn below(&self) -> Self {
        *self + ivec3(0, 0, -1)
    }

    fn is_solid(&self, _r: &impl Environs) -> bool {
        todo!()
    }

    fn can_be_entered(&self, _r: &impl Environs) -> bool {
        todo!()
    }

    fn tile(&self, r: &impl Environs) -> Tile {
        match (
            self.above().is_solid(r),
            self.is_solid(r),
            self.below().is_solid(r),
        ) {
            // Solid topside stack, makes a proper wall.
            //
            // Look for a voxel with an exposed side to show as wall.
            (true, true, _) => Tile::Solid(r.voxel(self).unwrap()),
            // Raised floor.
            //(false, true, _) => Some(Tile::Floor(self.voxel(r).unwrap())),
            (false, true, _) => Tile::Floor {
                block: r.voxel(self).unwrap(),
                z: 1,
                connectivity: self.above().high_connectivity(r),
            },
            // Regular floor
            (_, false, true) => Tile::Floor {
                block: r.voxel(&self.below()).unwrap(),
                z: 0,
                connectivity: 0,
            },
            // Depressed floor, check further down if there's surface.
            (_, _, false) => {
                if let Some(block) = r.voxel(&self.below().below()) {
                    Tile::Floor {
                        block,
                        z: -1,
                        connectivity: self.below().low_connectivity(r),
                    }
                } else {
                    Tile::Void
                }
            }
        }
    }

    fn cliff_form(&self, r: &impl Environs) -> Option<usize> {
        fn is_cliff(loc: &Location, r: &impl Environs) -> bool {
            matches!(loc.tile(r), Tile::Floor { z: 1, .. })
                && s8::ns(loc.truncate()).any(|a| {
                    matches!(a.extend(loc.z).tile(r), Tile::Floor { z: -1, .. })
                })
        }

        if is_cliff(self, r) {
            let mut mask = 0;
            for (i, loc) in s4::ns(self.truncate()).enumerate() {
                let loc = loc.extend(self.z);

                if is_cliff(&loc, r) {
                    mask |= 1 << i;
                }
            }
            // XXX: Seems like you get mostly artifacts if the cliff bits seem
            // fully unconnected.
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

    fn trace(&self, dir: IVec2) -> impl Iterator<Item = Self> {
        assert!(dir != IVec2::ZERO);

        let mut p = *self;
        std::iter::from_fn(move || {
            p += dir.extend(0);
            Some(p)
        })
    }
}

pub trait Environs {
    fn tile(&self, loc: &Location) -> Tile2D;
    fn set_tile(&mut self, loc: &Location, tile: Tile2D);

    fn voxel(&self, loc: &Location) -> Voxel;
}

impl Environs for Cloud<3, Tile2D> {
    fn tile(&self, loc: &Location) -> Tile2D {
        util::HashMap::get(self, &<[i32; 3]>::from(*loc))
            .copied()
            .unwrap_or_default()
    }

    fn set_tile(&mut self, loc: &Location, tile: Tile2D) {
        if tile == Default::default() {
            self.remove(*loc);
        } else {
            self.insert(*loc, tile);
        }
    }

    fn voxel(&self, _loc: &Location) -> Voxel {
        unimplemented!()
    }
}
