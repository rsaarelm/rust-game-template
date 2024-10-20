mod atlas;
pub use atlas::{Atlas, AtlasKey, BitAtlas};

mod block;
pub use block::{Block, Terrain, Tile, Voxel};

mod data;
pub use data::{
    register_data, register_data_from, settings, Data, EquippedAt, Item,
    ItemKind, Monster, Pod, PodKind, PodObject, Power, Region, Scenario,
    Settings, SpawnDist,
};

mod location;
use glam::{ivec3, IVec3};
pub use location::{Coordinates, Environs, Location};

pub mod mapgen;
pub use mapgen::{Lot, MapGenerator, Patch};

pub mod sector_map;
pub use sector_map::SectorMap;

mod world;
pub use world::{Level, World};

mod zone;
pub use zone::Zone;

pub type Rect = util::Rect<i32>;
pub type Cube = util::Cube<i32>;

/// Width of a single sector of the game world in tiles.
pub const SECTOR_WIDTH: i32 = 48;
/// Height of a single sector of the game world in tiles.
pub const SECTOR_HEIGHT: i32 = 40;
/// Depth of a single sector of the game world in tiles.
pub const LEVEL_DEPTH: i32 = 2;

pub const LEVEL_BASIS: IVec3 = ivec3(SECTOR_WIDTH, SECTOR_HEIGHT, LEVEL_DEPTH);

pub const EAST: IVec3 = ivec3(1, 0, 0);
pub const WEST: IVec3 = ivec3(-1, 0, 0);
pub const NORTH: IVec3 = ivec3(0, -1, 0);
pub const SOUTH: IVec3 = ivec3(0, 1, 0);
pub const DOWN: IVec3 = ivec3(0, 0, -1);
pub const UP: IVec3 = ivec3(0, 0, 1);
