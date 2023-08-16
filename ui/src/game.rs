use navni::prelude::*;

use engine::prelude::*;
use gfx::prelude::*;

const WIDTH: u32 = 160;
const HEIGHT: u32 = 48;

/// Toplevel context object for game state.
pub struct Game {
    /// Logic level data.
    pub r: Runtime,
    /// Display buffer.
    pub s: Buffer<CharCell>,
}

impl Default for Game {
    fn default() -> Self {
        Game {
            r: Default::default(),
            s: Buffer::new(WIDTH, HEIGHT),
        }
    }
}

impl Game {
    pub fn draw(&self, b: &mut dyn navni::Backend) {
        b.draw_chars(
            self.s.width() as _,
            self.s.height() as _,
            self.s.as_ref(),
        );
    }
}
