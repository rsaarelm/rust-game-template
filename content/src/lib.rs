#![feature(int_roundings)]

mod atlas;
pub use atlas::{Atlas, BitAtlas};

mod block;
pub use block::{Block, Tile, Voxel};

mod data;
pub use data::{
    register_mods, Data, EquippedAt, Item, ItemKind, Monster, Power, Scenario,
    SectorMap, Spawn, SpawnDist,
};

mod location;
pub use location::{Environs, LocExt, Location};

mod mapgen;
pub use mapgen::{Lot, MapGenerator, Patch};

mod tile;
pub use tile::{Terrain, Tile2D};

mod world;
pub use world::{Sector, World};

pub type Rect = util::Rect<i32>;
pub type Cube = util::Cube<i32>;

/// Width of a single sector of the game world in tiles.
pub const SECTOR_WIDTH: i32 = 52;
/// Height of a single sector of the game world in tiles.
pub const SECTOR_HEIGHT: i32 = 39;
/// Depth of a single sector of the game world in tiles.
pub const SECTOR_DEPTH: i32 = 1; // TODO: Change to 2 when going voxel
