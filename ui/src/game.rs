use crate::{prelude::*, Particle};
use engine::prelude::*;
use util::{s4, s8, Layout};

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

    selection: Vec<Entity>,

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
            selection: Default::default(),
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

        // If player doesn't exist, player is not acting this frame or player
        // is executing a goal, run in real time.
        if self.r.player().map_or(true, |p| {
            !p.acts_this_frame(&self.r) || p.goal(&self.r).is_some()
        }) {
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
                    for d in s8::DIR {
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

    /// Cycle selection to next commandable NPC.
    /// If at the end of the cycle, return selection to player.
    pub fn select_next_commandable(&mut self, skip_mobs_on_mission: bool) {
        let r = &self.r;
        let mut seen = 0;
        for mob in r
            .live_entities()
            .filter(|e| e.is_player_aligned(r) && !e.is_player(r))
        {
            let is_valid = if skip_mobs_on_mission {
                mob.is_waiting_commands(r)
            } else {
                true
            };

            // Past all items in current selection, pick this one.
            if seen == self.selection.len() && is_valid {
                self.selection = vec![mob];
                return;
            }

            // Otherwise keep tracking currently selected mobs until all are
            // accounted for.
            if self.selection.contains(&mob) {
                seen += 1;
            }
        }

        // No more commandable mobs found, go back to player.
        self.selection.clear();
    }

    pub fn current_active(&self) -> Option<Entity> {
        let p = self.r.player();

        if self.selection.is_empty()
            || self.selection.iter().find(|&&a| Some(a) == p).is_some()
        {
            // No selection means that player is active.
            // If the selection contains the player, always pick player.
            self.r.player()
        } else {
            // Just pick the first mob from a non-player selection.
            Some(self.selection[0])
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

    pub fn act(&mut self, cmd: impl Into<Command>) {
        match (cmd.into(), self.current_active()) {
            (Command::Direct(act), Some(p)) => {
                let r = &mut self.r;

                if p.is_player(r) {
                    // Player gets goals cleared by default.
                    p.clear_goal(r);
                } else {
                    // NPCs follow player by default, so switch to that.
                    p.set_goal(r, Goal::FollowPlayer);
                }

                if p.is_player(r) {
                    // Main player just does the thing.
                    p.execute(r, act);
                } else if p.can_be_commanded(r) {
                    // It's a NPC that still has actions left. Executing the
                    // action won't advance clock.
                    p.execute(r, act);

                    // If this action exhausted the actions, automatically
                    // cycle to the next NPC.
                    if !p.can_be_commanded(r) {
                        self.select_next_commandable(true);
                    }
                } else {
                    // Commanding a NPC past its actions makes it become the
                    // new main player.
                    p.become_player(r);
                    p.execute(r, act);
                }
            }
            (Command::Indirect(goal), Some(p)) => {
                let mut units = self.selection.clone();
                if units.is_empty() {
                    // No explicit selection, it's just the player then.
                    units.push(p);
                }

                let r = &mut self.r;

                for p in units {
                    p.set_goal(r, goal);
                    p.exhaust_actions(r);
                }
            }
            _ => {}
        }
    }

    pub fn process_action(&mut self, action: InputAction) {
        use InputAction::*;
        match action {
            North => self.act(Action::Bump(s4::DIR[0])),
            East => self.act(Action::Bump(s4::DIR[1])),
            South => self.act(Action::Bump(s4::DIR[2])),
            West => self.act(Action::Bump(s4::DIR[3])),
            FireNorth => {}
            FireEast => {}
            FireSouth => {}
            FireWest => {}
            SouthEast => {}
            SouthWest => {}
            NorthWest => {}
            NorthEast => {}
            ClimbUp => {}
            ClimbDown => {}
            LongMove => {}
            Cycle => self.select_next_commandable(false),
            Pass => self.act(Action::Pass),
            Inventory => {}
            Abilities => {}
            Equip => {}
            Unequip => {}
            Drop => {}
            Throw => {}
            Use => {}
            QuitGame => {}
            Cancel => {
                if let Some(p) = self.current_active() {
                    if p.is_player(&self.r) {
                        p.clear_goal(&mut self.r);
                    } else {
                        p.set_goal(&mut self.r, Goal::FollowPlayer);
                    }
                }
                self.selection = Default::default();
            }
            Autoexplore => {
                let r = &self.r;
                if let Some(p) = self.current_active() {
                    if let Some(enemy) = p.first_visible_enemy(r) {
                        // Autofight instead of autoexploring when there are
                        // visible enemies.
                        if let Some(atk) = p.decide(r, Goal::Attack(enemy)) {
                            self.act(atk);
                        }
                    } else {
                        self.act(Goal::StartAutoexplore);
                    }
                }
            }
            Quicksave => {}
            Quickload => {}
        }
    }

    pub fn set_selection(&mut self, sel: impl IntoIterator<Item = Entity>) {
        self.selection = sel.into_iter().collect();
    }

    pub fn selection(&self) -> &[Entity] {
        &self.selection
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Command {
    Direct(Action),
    Indirect(Goal),
}

impl From<Action> for Command {
    fn from(value: Action) -> Self {
        Command::Direct(value)
    }
}

impl From<Goal> for Command {
    fn from(value: Goal) -> Self {
        Command::Indirect(value)
    }
}
