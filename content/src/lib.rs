#![feature(int_roundings)]

mod atlas;
pub use atlas::{Atlas, BitAtlas};

mod data;
pub use data::{
    register_mods, Data, EquippedAt, Item, ItemKind, Monster, Power,
};

mod location;
pub use location::{LocExt, Location};

mod tile;
pub use tile::{Terrain, Tile};

mod world;
pub use world::{Sector, World};

pub type Rect = util::Rect<i32>;
pub type Cube = util::Cube<i32>;

/// Width of a single sector of the game world in tiles.
pub const SECTOR_WIDTH: i32 = 52;
/// Height of a single sector of the game world in tiles.
pub const SECTOR_HEIGHT: i32 = 39;
/// Depth of a single sector of the game world in tiles.
pub const SECTOR_DEPTH: i32 = 2;
