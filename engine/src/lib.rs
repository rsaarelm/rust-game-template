//! Game logic layer machinery.

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

pub mod ecs;

mod entity;
pub use entity::Entity;

mod entity_spec;
pub use entity_spec::EntitySpec;

mod fov;
pub use crate::fov::Fov;

mod item;

mod location;
pub use location::RuntimeCoordinates;

mod mob;
pub use mob::Buff;

mod msg;
pub use msg::{Grammatize, Msg, Receiver, send_msg};

mod pathing;
pub use pathing::FogPathing;

mod placement;
pub use placement::Placement;

pub mod power;

pub mod prelude;

mod runtime;
pub use runtime::Runtime;

mod time;
pub use time::Instant;

pub enum ScenarioStatus {
    Ongoing,
    Won,
    Lost,
}
