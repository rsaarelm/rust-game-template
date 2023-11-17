//! Game logic layer machinery.
#![feature(int_roundings)]
#![feature(lazy_cell)]

/// Width of a single sector of the game world in tiles.
pub const SECTOR_WIDTH: i32 = 52;
/// Height of a single sector of the game world in tiles.
pub const SECTOR_HEIGHT: i32 = 39;

/// How far can the player see.
pub const FOV_RADIUS: i32 = 10;

/// From how far away do inert enemies first react to foes.
pub const ALERT_RADIUS: i32 = 9;

/// From how far away does the enemy shout wake up mobs.
pub const SHOUT_RADIUS: i32 = 6;

/// How far can you throw items.
pub const THROW_RANGE: i32 = 10;

/// How many move phases does a complete turn contain.
pub const PHASES_IN_TURN: i64 = 12;

mod action;
pub use action::Action;

mod ai;
pub use ai::Goal;

mod atlas;
pub use atlas::{Atlas, BitAtlas};

mod data;
pub use data::{register_mods, Data, EntitySeed};

pub mod ecs;

mod entity;
pub use entity::Entity;

mod fov;
pub use crate::fov::Fov;

mod item;
pub use item::EquippedAt;

mod location;
pub use location::{Location, SectorDir};

mod mapgen;

mod mob;
pub use mob::Buff;

mod msg;
pub use msg::{send_msg, Grammatize, Msg, Receiver};

mod patch;
pub use patch::{FlatPatch, Patch, Spawn};

mod placement;
pub use placement::Placement;

pub mod power;
pub use power::Power;

pub mod prelude;

mod runtime;
pub use runtime::Runtime;

mod terrain;
pub use terrain::TileTerrain;

mod tile;
pub use tile::MapTile;

mod time;
pub use time::Instant;

mod world;
pub use world::{World, WorldSpec};

pub type Rect = util::Rect<i32>;
pub type Cube = util::Cube<i32>;

pub enum ScenarioStatus {
    Ongoing,
    Won,
    Lost,
}
