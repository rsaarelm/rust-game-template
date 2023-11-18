use derive_more::{Add, Deref};
use glam::{ivec3, IVec3};
use serde::{Deserialize, Serialize};
use util::Logos;

use crate::{
    Cube, Location, Terrain, SECTOR_DEPTH, SECTOR_HEIGHT, SECTOR_WIDTH,
};

// TODO Retire engine::World, use this instead
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct World {
    seed: Logos,

    // TODO spawn history
    /// Terrain that has been changed at runtime, saved in savefile.
    overlay: Terrain,
    // TODO scenario data
    #[serde(skip)]
    /// Procedurally generated main terrain store.
    ///
    /// Not saved in savefiles, can be regenerated at any time from scenario
    /// data.
    terrain_cache: Terrain,
}

#[derive(
    Copy, Clone, Eq, PartialEq, Hash, Debug, Deref, Add, Serialize, Deserialize,
)]
pub struct Sector(IVec3);

impl From<IVec3> for Sector {
    fn from(value: IVec3) -> Self {
        Sector(ivec3(
            (value.x).div_floor(SECTOR_WIDTH),
            (value.y).div_floor(SECTOR_HEIGHT),
            (value.z).div_floor(SECTOR_DEPTH),
        ))
    }
}

impl From<Sector> for IVec3 {
    fn from(value: Sector) -> Self {
        ivec3(
            value.x * SECTOR_WIDTH,
            value.y * SECTOR_HEIGHT,
            value.z * SECTOR_DEPTH,
        )
    }
}

impl From<Sector> for Cube {
    fn from(value: Sector) -> Self {
        let sector_size = ivec3(SECTOR_WIDTH, SECTOR_HEIGHT, SECTOR_DEPTH);
        let origin = *value * ivec3(SECTOR_WIDTH, SECTOR_HEIGHT, SECTOR_DEPTH);
        Cube::new(origin, origin + sector_size)
    }
}

impl Sector {
    /// Return the sector neighborhood which should have maps generated for it
    /// when the central sector is being set up as an active play area.
    pub fn cache_volume(&self) -> impl Iterator<Item = Sector> {
        let s = *self;
        // All 8 chess-metric neighbors plus above and below sectors. Should
        // be enough to cover everything needed while moving around the center
        // sector.
        [
            ivec3(0, 0, 0),
            ivec3(0, -1, 0),
            ivec3(1, -1, 0),
            ivec3(1, 0, 0),
            ivec3(1, 1, 0),
            ivec3(0, 1, 0),
            ivec3(-1, 1, 0),
            ivec3(-1, 0, 0),
            ivec3(-1, -1, 0),
            ivec3(0, 0, -1),
            ivec3(0, 0, 1),
        ]
        .into_iter()
        .map(move |d| s + Sector(d))
    }
}
