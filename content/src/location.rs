use glam::{ivec3, IVec3};

use crate::{SECTOR_HEIGHT, SECTOR_WIDTH};

pub type Location = IVec3;

/// Methods for points when treated as game world locations.
pub trait LocExt {
    /// Snap location to origin of it's current 2D sector-slice.
    fn sector_snap_2d(&self) -> Self;
}

impl LocExt for Location {
    fn sector_snap_2d(&self) -> Self {
        ivec3(
            self.x.div_floor(SECTOR_WIDTH) * SECTOR_WIDTH,
            self.y.div_floor(SECTOR_HEIGHT) * SECTOR_HEIGHT,
            self.z,
        )
    }
}
