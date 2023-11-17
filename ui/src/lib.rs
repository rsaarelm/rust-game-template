//! Game user interface machinery

pub mod prelude {
    use navni::prelude::*;

    pub use crate::{
        anim::Anim, cell, game, input_press, Backdrop, Cursor, Game,
        InputAction, Widget, Window,
    };

    pub type Buffer = gfx::Buffer<CharCell>;
    pub type Rect = util::Rect<i32>;
}

pub mod anim;

mod backdrop;
pub use backdrop::Backdrop;

mod command;
use command::Command;

mod cursor;
pub use cursor::Cursor;

mod game;
pub use game::{game, init_game, Game};

mod input;
pub use input::{input_press, InputAction, InputMap};

mod tile_display;
pub use tile_display::{flat_terrain_cell, terrain_cell};

mod widget;
pub use widget::{Centered, ConfirmationDialog, Widget};

mod window;
pub use window::Window;

pub fn cell(
    c: char,
    fore: impl Into<navni::X256Color>,
    back: impl Into<navni::X256Color>,
) -> navni::CharCell {
    navni::CharCell::new(c, fore, back)
}

pub const LIGHT_PALETTE: [navni::Rgba; 16] = {
    use navni::Rgba;
    [
        Rgba::new(0xaa, 0xaa, 0xaa, 0xff), // white
        Rgba::new(0x66, 0x00, 0x00, 0xff), // maroon
        Rgba::new(0x00, 0x66, 0x00, 0xff), // green
        Rgba::new(0x66, 0x33, 0x00, 0xff), // brown
        Rgba::new(0x00, 0x00, 0x88, 0xff), // navy
        Rgba::new(0x66, 0x00, 0x66, 0xff), // purple
        Rgba::new(0x00, 0x66, 0x66, 0xff), // teal
        Rgba::new(0x33, 0x33, 0x33, 0xff), // gray
        Rgba::new(0x77, 0x77, 0x77, 0xff), // silver
        Rgba::new(0xaa, 0x00, 0x00, 0xff), // red
        Rgba::new(0x00, 0xaa, 0x00, 0xff), // lime
        Rgba::new(0xaa, 0x55, 0x00, 0xff), // yellow
        Rgba::new(0x22, 0x22, 0xee, 0xff), // blue
        Rgba::new(0xaa, 0x00, 0xaa, 0xff), // fuchsia
        Rgba::new(0x00, 0x99, 0x99, 0xff), // aqua
        Rgba::new(0x00, 0x00, 0x00, 0xff), // black
    ]
};
