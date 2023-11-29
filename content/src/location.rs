use glam::{ivec3, IVec2, IVec3};
use util::Cloud;

use crate::{Tile, SECTOR_HEIGHT, SECTOR_WIDTH};

pub type Location = IVec3;

/// Methods for points when treated as game world locations.
pub trait LocExt: Sized {
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
}

impl LocExt for Location {
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
}

pub trait Environs {
    fn tile(&self, loc: Location) -> Tile;
    fn set_tile(&mut self, loc: Location, tile: Tile);
}

impl Environs for Cloud<3, Tile> {
    fn tile(&self, loc: Location) -> Tile {
        util::HashMap::get(self, &<[i32; 3]>::from(loc))
            .copied()
            .unwrap_or_default()
    }

    fn set_tile(&mut self, loc: Location, tile: Tile) {
        if tile == Default::default() {
            self.remove(loc);
        } else {
            self.insert(loc, tile);
        }
    }
}
