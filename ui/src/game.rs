use anyhow::Result;
use engine::prelude::*;
use navni::{prelude::*, X256Color as X};
use util::{s4, s8, Layout, SameThread};

use crate::{anim, prelude::*, Command, InputMap};

// Maximum GUI terminal size.
// Get just about to a size where a whole sector fits on map screen.
const WIDTH: u32 = 150;
const HEIGHT: u32 = 45;

/// Toplevel context object for game state.
pub struct Game {
    same_thread: SameThread,

    /// Logic level data.
    pub r: Runtime,
    /// Display buffer.
    pub s: Buffer,

    /// Current viewpoint position, the mob that's being followed.
    pub viewpoint: Location,
    /// Camera position on screen, can be scrolled away from viewpoint.
    pub camera: Location,

    selection: Vec<Entity>,
    pub planned_path: PlannedPath,

    /// Receiver for engine events.
    recv: Receiver,
    pub msg: Vec<String>,

    /// Animations below the fog of war.
    ground_anims: Vec<Box<dyn Anim>>,
    /// Animations above the fog of war.
    sky_anims: Vec<Box<dyn Anim>>,

    pub input_map: InputMap,
}

static mut GAME: Option<Game> = None;

pub fn init_game() {
    unsafe {
        GAME = Some(Game::default());
    }
}

pub fn game() -> &'static mut Game {
    let ret = unsafe { GAME.as_mut().expect("game not initialized") };
    ret.same_thread.assert();
    ret
}

impl AsRef<Runtime> for Game {
    fn as_ref(&self) -> &Runtime {
        &self.r
    }
}

impl AsMut<Runtime> for Game {
    fn as_mut(&mut self) -> &mut Runtime {
        &mut self.r
    }
}

impl AsRef<Buffer> for Game {
    fn as_ref(&self) -> &Buffer {
        &self.s
    }
}

impl AsMut<Buffer> for Game {
    fn as_mut(&mut self) -> &mut Buffer {
        &mut self.s
    }
}

impl Default for Game {
    fn default() -> Self {
        let layout = Layout::system_layout();
        log::info!("detected {layout:?} keyboard layout");
        let input_map = InputMap::for_layout(Layout::system_layout());

        Game {
            same_thread: Default::default(),
            r: Default::default(),
            s: Buffer::new(WIDTH, HEIGHT),
            viewpoint: Default::default(),
            camera: Default::default(),
            selection: Default::default(),
            planned_path: Default::default(),
            recv: Default::default(),
            msg: Default::default(),
            ground_anims: Default::default(),
            sky_anims: Default::default(),
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

    pub fn tick(&mut self) {
        // Clear the dead from selection.
        for i in (0..self.selection.len()).rev() {
            if !self.selection[i].is_alive(self) {
                self.selection.swap_remove(i);
            }
        }

        // If player doesn't exist, player is not acting this frame or player
        // is executing a goal, run in real time.
        if self.r.player().map_or(true, |p| {
            !p.acts_this_frame(self) || p.goal(self).is_some()
        }) {
            self.r.tick();
        }

        // Update camera in case engine tick moved player.
        self.update_camera();

        // Clear message buffer if any key is pressed or the mouse is clicked.
        if navni::keypress().is_some()
            || matches!(navni::mouse_state(), MouseState::Release(_, _, _))
        {
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
                        anim::Particle::new(e, 10).offset(dir).c(dir.to_char()),
                    ));
                }
                Hurt(e) => {
                    self.add_anim(Box::new(
                        anim::Particle::new(e, 10).c('*').col(X::RED),
                    ));
                }
                Miss(e) => {
                    self.add_anim(Box::new(anim::Particle::new(e, 3).c('·')));
                }
                Death(loc) => {
                    for d in s8::DIR {
                        self.add_anim(Box::new(
                            anim::Particle::new(loc, 15)
                                .c('*')
                                .col(X::YELLOW)
                                .v(0.25 * d.as_vec2().normalize()),
                        ));
                    }
                }
                Explosion(loc) => {
                    self.add_anim(Box::new(anim::Explosion::new(loc)));
                }
                LightningBolt(loc) => {
                    // Only add sky animations if the player can see them.
                    if loc.is_explored(self) {
                        self.add_sky_anim(Box::new(anim::Lightning::new(loc)));
                    }
                }
                MagicMap(posns) => {
                    // Map revealed cells into wide space and lengthen the
                    // reveal times so we can insert in-between times for the
                    // side cells.
                    let posns: HashMap<IVec2, usize> = posns
                        .into_iter()
                        .map(|(loc, n)| (loc.unfold_wide(), n * 2))
                        .collect();

                    // Fill the middle positions between two revealed cells.
                    let sides: Vec<(IVec2, usize)> = posns
                        .iter()
                        .filter_map(|(&loc, n)| {
                            posns
                                .get(&(loc + ivec2(2, 0)))
                                .map(|n2| (loc + ivec2(1, 0), n.min(n2) + 1))
                        })
                        .collect();

                    for (p, mut t) in posns.into_iter().chain(sides) {
                        self.add_anim(Box::new(
                            move |_: &Runtime, n, win: &Window, off| {
                                let p = p - off;
                                if t > 0 {
                                    win.put(p, CharCell::c('░').col(X::BROWN));
                                } else {
                                    win.put(p, CharCell::c('*').col(X::YELLOW));
                                }
                                anim::countdown(n, &mut t)
                            },
                        ));
                    }
                }
            }
        }
    }

    /// Cycle selection to next commandable NPC.
    /// If at the end of the cycle, return selection to player.
    pub fn select_next_commandable(&mut self, only_ones_waiting_orders: bool) {
        let r = &self.r;
        let mut seen = 0;
        for mob in r
            .live_entities()
            .filter(|e| e.is_player_aligned(r) && !e.is_player(r))
        {
            let is_valid = if only_ones_waiting_orders {
                mob.acts_before_next_player_frame(r)
                    && mob.is_waiting_commands(r)
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
            || self.selection.iter().any(|&a| Some(a) == p)
        {
            // No selection means that player is active.
            // If the selection contains the player, always pick player.
            self.r.player()
        } else {
            // Just pick the first mob from a non-player selection.
            Some(self.selection[0])
        }
    }

    pub fn draw_ground_anims(&mut self, win: &Window, draw_offset: IVec2) {
        draw_anims(&self.r, win, draw_offset, &mut self.ground_anims);
    }

    pub fn draw_sky_anims(&mut self, win: &Window, draw_offset: IVec2) {
        draw_anims(&self.r, win, draw_offset, &mut self.sky_anims);
    }

    pub fn add_anim(&mut self, anim: Box<dyn Anim>) {
        self.ground_anims.push(anim);
    }

    pub fn add_sky_anim(&mut self, anim: Box<dyn Anim>) {
        self.sky_anims.push(anim);
    }

    /// The async bottom point where things get actually drawn to screen.
    ///
    /// If the screen got resized, all the layout must be done again. As a
    /// hack around this, `draw` will return `None` on a resize that can be
    /// caught at a higher level and used to abort a stack of modal options.
    pub async fn draw(&mut self) -> Option<()> {
        // Check for window resize
        let (w, h) = navni::char_resolution(WIDTH, HEIGHT);
        let mut was_resized = false;

        if w != 0
            && h != 0
            && (self.s.width() != w as i32 || self.s.height() != h as i32)
        {
            self.s = Buffer::new(w, h);

            // Reset scroll when resized.
            self.camera = self.viewpoint;

            // Signal the caller that the screen layout has been
            // invalidated.
            was_resized = true;
        }

        navni::draw_chars(
            self.s.width() as _,
            self.s.height() as _,
            self.s.as_ref(),
        )
        .await;

        (!was_resized).then_some(())
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
                    p.execute_direct(r, act);
                } else if p.can_be_commanded(r) {
                    // It's a NPC that still has actions left. Executing the
                    // action won't advance clock.
                    p.execute_direct(r, act);

                    // If this action exhausted the actions, automatically
                    // cycle to the next NPC.
                    if !p.can_be_commanded(r) {
                        self.select_next_commandable(true);
                    }
                } else {
                    // Commanding a NPC past its actions makes it become the
                    // new main player.
                    p.become_player(r);
                    p.execute_direct(r, act);
                }
            }
            (Command::Indirect(Goal::GoTo(loc)), Some(_)) => {
                if self.player_is_selected() {
                    // For player group, player gets the goal, others follow
                    // player.

                    let Some(p) = self.r.player() else { return };
                    let Some(start) = p.loc(&self.r) else { return };

                    for e in self.selection.iter() {
                        if !e.is_player(&self.r) {
                            e.set_goal(&mut self.r, Goal::FollowPlayer);
                        }
                    }

                    if p.is_threatened(&self.r) {
                        // If player is threatened, see if it looks like
                        // you're trying to fight or flee.
                        let Some(mut planned_path) =
                            self.r.fov_aware_path_to(&start, &loc)
                        else {
                            return;
                        };

                        let Some(step) = planned_path.pop() else {
                            return;
                        };
                        let Some(dir) = start.vec_towards(&step).map(s4::norm)
                        else {
                            return;
                        };

                        if p.is_threatened_from(&self.r, dir) {
                            // Pointing towards the enemy, fight instead of
                            // moving.
                            p.clear_goal(&mut self.r);
                            self.autofight(p);
                            return;
                        }
                    }
                    p.set_goal(&mut self.r, Goal::GoTo(loc));
                } else {
                    // Non-player group: Everyone gets an attack-move command,
                    // will return to following player when done.
                    for p in self.selection.iter() {
                        p.set_goal(&mut self.r, Goal::AttackMove(loc));
                        p.exhaust_actions(&mut self.r);
                    }
                    self.select_next_commandable(true);
                }
            }
            (Command::Indirect(Goal::StartAutoexplore), Some(p)) => {
                if !self.player_is_selected() {
                    for e in self.selected().collect::<Vec<_>>() {
                        e.set_goal(&mut self.r, Goal::Autoexplore);
                        e.exhaust_actions(&mut self.r);
                    }
                    self.select_next_commandable(true);
                } else {
                    debug_assert!(p.is_player(&self.r));
                    // Player can do the adjacent sector search with
                    // StartAutoexplore, NPCs just get regular autoexplore.
                    p.set_goal(&mut self.r, Goal::StartAutoexplore);

                    for e in self.selected().collect::<Vec<_>>() {
                        // Set the others as escorts when player is doing the
                        // main action.
                        if e != p {
                            e.set_goal(&mut self.r, Goal::FollowPlayer);
                        }
                    }
                    self.clear_selection();
                }
            }
            (Command::Indirect(goal), Some(_)) => {
                // TODO: Do other indirect commands need a mode for when the player character is also doing it?
                if !self.player_is_selected() {
                    for p in self.selected().collect::<Vec<_>>() {
                        p.set_goal(&mut self.r, goal);
                        p.exhaust_actions(&mut self.r);
                    }
                    self.select_next_commandable(true);
                }
            }
            _ => {}
        }

        // Player may have moved, check camera position.
        self.update_camera();
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
            BecomePlayer => {
                if let Some(p) = self.current_active() {
                    p.become_player(&mut self.r);
                }
            }
            Pass => self.act(Action::Pass),
            /*
            Inventory => {
                if let Some(p) = self.current_active() {
                    if p.inventory(self).next().is_some() {
                        self.cmd = CommandState::Partial(Part::ViewInventory);
                    } else {
                        msg!("[One] [is] not carrying anything."; p.noun(self));
                    }
                }
            }
            */
            Inventory => {}
            Powers => {}
            Equipment => {}
            Drop => {}
            Throw => {}
            Use => {}
            QuitGame => {}
            Cancel => {
                if let Some(p) = self.current_active() {
                    if p.is_player(self) {
                        p.clear_goal(&mut self.r);
                    } else {
                        p.set_goal(&mut self.r, Goal::FollowPlayer);
                    }
                }
                self.selection = Default::default();
            }
            Roam => {
                if let Some(p) = self.current_active() {
                    if !self.autofight(p) {
                        self.act(Goal::StartAutoexplore);
                    }
                }
            }
            ScrollNorth => {}
            ScrollEast => {}
            ScrollSouth => {}
            ScrollWest => {}
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    pub fn set_selection(&mut self, sel: impl IntoIterator<Item = Entity>) {
        self.selection = sel.into_iter().collect();

        // Just the player amounts to no selection.
        if self.selection.len() == 1 && self.selection[0].is_player(self) {
            self.selection.clear();
        }
    }

    pub fn selected(&self) -> impl Iterator<Item = Entity> + '_ {
        self.selection
            .iter()
            .copied()
            .chain(if self.selection.is_empty() {
                self.current_active()
            } else {
                None
            })
    }

    pub fn player_is_selected(&self) -> bool {
        self.selection.is_empty()
            || self.selection.iter().any(|p| p.is_player(self))
    }

    pub fn autofight(&mut self, p: Entity) -> bool {
        if let Some(enemy) = p.first_visible_enemy(self) {
            if let Some(atk) = p.decide(self, Goal::Attack(enemy)) {
                self.act(atk);
                return true;
            }
        }
        false
    }

    pub fn save(&mut self, game_name: &str) {
        let saved =
            idm::to_string(&self.r).expect("runtime serialization failed");
        navni::Directory::data(game_name)
            .expect("data dir not found")
            .write("saved.idm", &saved)
            .expect("writing save failed");
    }

    pub fn delete_save(&self, game_name: &str) {
        if navni::Directory::data(game_name)
            .expect("data dir not found")
            .exists("saved.idm")
        {
            navni::Directory::data(game_name)
                .expect("data dir not found")
                .delete("saved.idm")
                .expect("deleting save failed");
        }
    }

    /// Return Ok(Some(save)) if save file is found and parsed successfully.
    /// Return Ok(None) if there is no save file. Return an error if save file
    /// is present but could not be parsed.
    pub fn load(&mut self, game_name: &str) -> Result<Option<Runtime>> {
        if let Ok(save) = navni::Directory::data(game_name)
            .expect("data dir not found")
            .read("saved.idm")
        {
            // Return an error if deserialization fails.
            match idm::from_str(&save) {
                Err(e) => Err(e.into()),
                Ok(r) => Ok(Some(r)),
            }
        } else {
            Ok(None)
        }
    }

    pub fn replace_runtime(&mut self, r: Runtime) {
        self.r = r;

        // If player was in the middle of a long action when game was saved,
        // abort that. It's confusing to load back into game where the player
        // is running around.
        if let Some(p) = self.current_active() {
            p.clear_goal(self);
        }
    }

    fn update_camera(&mut self) {
        if let Some(loc) = self.current_active().and_then(|p| p.loc(self)) {
            if loc != self.viewpoint {
                self.viewpoint = loc;
                self.camera = self.viewpoint;
                self.planned_path.clear();
            }
        }
    }

    pub fn quit(&mut self) {
        while let Some(c) = self.current_active() {
            c.die(self, None);
        }
    }

    pub fn is_game_over(&self) -> bool {
        self.current_active().is_none()
    }
}

fn draw_anims(
    r: &impl AsRef<Runtime>,
    win: &Window,
    draw_offset: IVec2,
    set: &mut Vec<Box<dyn Anim>>,
) {
    let n_updates = navni::logical_frames_elapsed();
    let r = r.as_ref();
    for i in (0..set.len()).rev() {
        // Iterate anims backwards so when we swap-remove expired
        // animations this doesn't affect upcoming elements.
        if !set[i].render(r, n_updates, win, draw_offset) {
            set.swap_remove(i);
        }
    }
}

#[derive(Default)]
pub struct PlannedPath {
    posns: Vec<IVec2>,
    mouse_pos: IVec2,
}

impl PlannedPath {
    pub fn clear(&mut self) {
        self.posns.clear();
    }

    pub fn update(
        &mut self,
        r: &impl AsRef<Runtime>,
        orig: Location,
        dest: Location,
        mouse_pos: impl Into<IVec2>,
    ) {
        let r = r.as_ref();

        let mouse_pos = mouse_pos.into();
        // Don't update until mouse actually moves.
        if mouse_pos == self.mouse_pos {
            return;
        }
        self.mouse_pos = mouse_pos;

        self.posns.clear();
        if let Some(mut path) = r.fov_aware_path_to(&orig, &dest) {
            if !path.is_empty() {
                // If the path goes down a stairwell,
                // the actual endpoint will be on
                // another sector and won't be shown.
                // Tweak the visible path so it's all
                // on the local screen.
                path[0] = dest;
            } else {
                return;
            }

            self.posns.push(path[0].unfold_wide());

            for w in path.windows(2) {
                let [a, c] = w else { continue };
                let (a, c) = (a.unfold_wide(), c.unfold_wide());

                // The in-between part for horizontal step.
                let b = a + (c - a) / ivec2(2, 2);

                if b != self.posns[self.posns.len() - 1] {
                    self.posns.push(b);
                }

                if c != self.posns[self.posns.len() - 1] {
                    self.posns.push(c);
                }
            }
        }
    }

    pub fn posns(&self) -> &[IVec2] {
        &self.posns
    }
}
