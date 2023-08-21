use crate::{prelude::*, Particle};
use engine::prelude::*;
use util::Layout;

use navni::X256Color as X;

use crate::{Anim, InputMap};

// Target size, looks nice on a 1080p display.
const WIDTH: u32 = 120;
const HEIGHT: u32 = 36;

/// Toplevel context object for game state.
pub struct Game {
    /// Logic level data.
    pub r: Runtime,
    /// Display buffer.
    pub s: Buffer,

    /// Current viewpoint.
    pub camera: Location,

    /// Receiver for engine events.
    recv: Receiver,
    pub msg: Vec<String>,

    anims: Vec<Box<dyn Anim>>,

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
            anims: Default::default(),
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
            use Msg::*;
            match msg {
                Message(text) => {
                    self.msg.push(text);
                }
                Fire(e, dir) => {
                    self.add_anim(Box::new(
                        Particle::new(e, 10).offset(dir).c(dir.to_char()),
                    ));
                }
                Hurt(e) => {
                    self.add_anim(Box::new(
                        Particle::new(e, 10).c('*').col(X::RED),
                    ));
                }
                Miss(e) => {
                    self.add_anim(Box::new(Particle::new(e, 3).c('·')));
                }
                Death(loc) => {
                    for d in DIR_8 {
                        self.add_anim(Box::new(
                            Particle::new(loc, 15)
                                .c('*')
                                .col(X::YELLOW)
                                .v(0.25 * d.as_vec2().normalize()),
                        ));
                    }
                }
            }
        }
    }

    pub fn draw_anims(
        &mut self,
        n_updates: u32,
        win: &Window,
        draw_offset: IVec2,
    ) {
        for i in (0..self.anims.len()).rev() {
            // Iterate anims backwards so when we swap-remove expired
            // animations this doesn't affect upcoming elements.
            if !self.anims[i].render(
                &self.r,
                &mut self.s,
                n_updates,
                win,
                draw_offset,
            ) {
                self.anims.swap_remove(i);
            }
        }
    }

    pub fn add_anim(&mut self, anim: Box<dyn Anim>) {
        self.anims.push(anim);
    }

    pub fn draw(&self, b: &mut dyn navni::Backend) {
        b.draw_chars(
            self.s.width() as _,
            self.s.height() as _,
            self.s.as_ref(),
        );
    }
}
