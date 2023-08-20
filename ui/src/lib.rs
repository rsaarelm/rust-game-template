//! Game user interface machinery

mod game;
pub use game::Game;

mod input;
pub use input::{InputAction, InputMap};

pub mod prelude;

mod tile_display;
pub use tile_display::terrain_cell;
