use navni::prelude::*;

use engine::prelude::*;
use gfx::prelude::*;
use util::Layout;

use crate::InputMap;

// Target size, looks nice on a 1080p display.
const WIDTH: u32 = 120;
const HEIGHT: u32 = 36;

/// Toplevel context object for game state.
pub struct Game {
    /// Logic level data.
    pub r: Runtime,
    /// Display buffer.
    pub s: Buffer<CharCell>,

    /// Current viewpoint.
    pub camera: Location,

    /// Receiver for engine events.
    recv: Receiver,
    pub msg: Vec<String>,

    pub input_map: InputMap,
}

impl Default for Game {
    fn default() -> Self {
        let layout = Layout::system_layout();
        log::info!("detected {layout:?} keyboard layout");
        let input_map = InputMap::for_layout(Layout::system_layout());

        Game {
            r: Default::default(),
            s: Buffer::new(WIDTH, HEIGHT),
            camera: Default::default(),
            recv: Default::default(),
            msg: Default::default(),
            input_map,
        }
    }
}

impl Game {
    pub fn new(runtime: Runtime) -> Self {
        Game {
            r: runtime,
            ..Default::default()
        }
    }

    pub fn tick(&mut self, b: &dyn navni::Backend) {
        // Check for window resize
        let (mut w, mut h) = b.char_resolution();
        if w == 0 || h == 0 {
            // Out of focus window probably, do nothing.
        } else {
            if b.is_gui() {
                // Don't go too tiny compared to target size.
                while w > WIDTH || h > HEIGHT {
                    w /= 2;
                    h /= 2;
                }
            }

            if self.s.width() != w as i32 || self.s.height() != h as i32 {
                self.s = Buffer::new(w, h);
            }
        }

        // Player is not waiting for input, update the world.
        if !self
            .r
            .player()
            .map_or(false, |p| p.acts_this_frame(&self.r))
        {
            self.r.tick();
        }

        // Clear message buffer if any key is pressed.
        if b.keypress().is_some() {
            self.msg.clear();
        }

        // Pump messages from world
        while let Ok(msg) = self.recv.try_recv() {
            match msg {
                Msg::Message(text) => {
                    self.msg.push(text);
                }
                Msg::Fire(_e, _dir) => {
                    // TODO: Fire animation
                }
                Msg::Hurt(_e) => {
                    // TODO: Hurt particle anim (and drop the msg)
                    self.msg.push(format!("{} is hit.", _e.Name(&self.r)));
                }
                Msg::Miss(_e) => {
                    // TODO: Particle anim for a blink on the mob
                }
                Msg::Death(_loc) => {
                    // TODO: Particle explosion at location.
                }
            }
        }
    }

    pub fn draw(&self, b: &mut dyn navni::Backend) {
        b.draw_chars(
            self.s.width() as _,
            self.s.height() as _,
            self.s.as_ref(),
        );
    }
}
