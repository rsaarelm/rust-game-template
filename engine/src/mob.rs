//! Entity logic for active creatures.
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use util::s4;

use crate::{
    ecs::{ActsNext, IsFriendly, IsMob, Momentum, Speed, Stats},
    prelude::*,
    EquippedAt, FOV_RADIUS, PHASES_IN_TURN, THROW_DIST,
};

impl Entity {
    pub fn is_player(&self, r: &Runtime) -> bool {
        r.player == Some(*self)
    }

    pub fn become_player(&self, r: &mut Runtime) {
        let prev_player = r.player();
        if Some(*self) == prev_player {
            return;
        }

        r.player = Some(*self);
        // Clear goal, existing ones probably won't make sense for a
        // player mob.
        self.clear_goal(r);

        if let Some(prev_player) = prev_player {
            // Give the previous player mob a follower AI so it won't just
            // stand around.
            if prev_player != *self {
                prev_player.set_goal(r, Goal::FollowPlayer);
            }
        }
    }

    pub fn is_mob(&self, r: &Runtime) -> bool {
        self.get::<IsMob>(r).0
    }

    pub fn is_player_aligned(&self, r: &Runtime) -> bool {
        self.get::<IsFriendly>(r).0
    }

    pub fn is_npc(&self, r: &Runtime) -> bool {
        self.is_player_aligned(r) && !self.is_player(r)
    }

    pub fn is_enemy(&self, r: &Runtime, other: &Entity) -> bool {
        self.is_player_aligned(r) != other.is_player_aligned(r)
    }

    pub fn is_ally(&self, r: &Runtime, other: &Entity) -> bool {
        self.is_player_aligned(r) == other.is_player_aligned(r)
    }

    /// Return the vector the mob is moving along "right now".
    ///
    /// Used for displace deadlocks, a mob with existing momentum shouldn't be
    /// displaced against the momentum vector.
    pub fn live_momentum(&self, r: &Runtime) -> IVec2 {
        if self.acts_next(r) > r.now() {
            // Only display momentum if the mob has moved during the current
            // phase.
            self.get::<Momentum>(r).0
        } else {
            Default::default()
        }
    }

    pub fn can_step(&self, r: &Runtime, dir: IVec2) -> bool {
        let Some(loc) = self.loc(r) else { return false };
        let n = (loc + dir).follow(r);

        if !n.is_walkable(r) {
            return false;
        }

        if let Some(mob) = n.mob_at(r) {
            if !self.can_displace(r, dir, &mob) {
                return false;
            }
        }

        true
    }

    pub fn can_displace(
        &self,
        r: &Runtime,
        dir: IVec2,
        other: &Entity,
    ) -> bool {
        if other.is_player(r) {
            // Don't displace the player.
            return false;
        }

        if !self.is_ally(r, other) {
            // Can't displace enemies.
            return false;
        }

        if self.is_player(r) {
            // Player can displace regardless of momentum.
            return true;
        }

        let m = other.live_momentum(r);
        if m != IVec2::ZERO && m.dot(-dir) <= 0 {
            // If there is live momentum, only allow displaces that push a
            // nonzero amount further towards the momentum vector (displace
            // push happens in direction opposite to dir). This avoids
            // deadlocks where two NPCs try move in opposing directions and
            // keep displacing each other.
            return false;
        }

        true
    }

    pub fn acts_next(&self, r: &Runtime) -> Instant {
        self.get::<ActsNext>(r).0
    }

    pub fn acts_this_frame(&self, r: &Runtime) -> bool {
        self.get::<Speed>(r).0 > 0 && self.acts_next(r) <= r.now()
    }

    pub fn acts_before_next_player_frame(&self, r: &Runtime) -> bool {
        if let Some(player) = r.player() {
            self.acts_next(r) <= player.next_phase_frame(r)
        } else {
            self.acts_this_frame(r)
        }
    }

    pub fn can_be_commanded(&self, r: &Runtime) -> bool {
        // NPCs can be commanded up to one full turn into the future.
        self.is_alive(r) && self.acts_next(r) - r.now() < PHASES_IN_TURN
    }

    pub fn is_waiting_commands(&self, r: &Runtime) -> bool {
        self.can_be_commanded(r)
            && matches!(self.goal(r), Goal::None | Goal::FollowPlayer)
    }

    /// Special method to immediately run goals on a NPC.
    ///
    /// Returns false if there is no goal or the NPC can't be commanded
    /// anymore on this turn.
    ///
    /// NB. Since this is meant for running explicit orders on NPCs, it does
    /// nothing and returns false if the goal is the default `FollowPlayer`.
    pub fn exhaust_actions(&self, r: &mut Runtime) {
        if !self.is_npc(r) {
            return;
        }

        // XXX: Goals might spin wheels forever, so add a release valve of
        // only spinning for a limited number of rounds.
        for _failsafe in 0..32 {
            let goal = self.goal(r);
            if !self.can_be_commanded(r) {
                break;
            }

            if matches!(goal, Goal::None | Goal::FollowPlayer) {
                break;
            }

            if let Some(act) = self.decide(r, goal) {
                self.execute_indirect(r, act);
            } else {
                self.next_goal(r);
            }
        }
    }

    pub(crate) fn next_phase_frame(&self, r: &Runtime) -> Instant {
        let mut t = self.acts_next(r).max(r.now()) + 1;
        let speed = self.get::<Speed>(r).0;
        assert!(speed > 0);

        while !t.is_action_frame(speed) {
            t += 1;
        }
        t
    }

    /// Decide on the next action given a goal.
    pub fn decide(&self, r: &Runtime, goal: Goal) -> Option<Action> {
        let mut dest;

        let loc = self.loc(r)?;

        match goal {
            Goal::None => return Some(Action::Pass),
            Goal::FollowPlayer => {
                let Some(player) = r.player() else {
                    return None;
                };

                if self.is_player(r) {
                    log::warn!(
                        "mob::decide: Player was assigned FollowPlayer goal"
                    );
                    return None;
                }

                if let Some(enemy_loc) =
                    self.first_visible_enemy(r).and_then(|e| e.loc(r))
                {
                    // Enemies visible! Go fight them.
                    dest = enemy_loc;
                } else if let Some(loc) = player.loc(r) {
                    // Otherwise follow player.
                    dest = loc;
                } else {
                    // Follow target can't be found, abandon goal.
                    return None;
                }
            }

            Goal::Autoexplore | Goal::StartAutoexplore => {
                let start = matches!(goal, Goal::StartAutoexplore);

                if !self.is_player(r) {
                    // Non-players fight when they run into enemies when
                    // exploring.
                    if let Some(e) = self.first_visible_enemy(r) {
                        return self.decide(r, Goal::Attack(e));
                    }
                }

                let explore_map = r.autoexplore_map(loc);
                if explore_map.is_empty() {
                    // If starting out and done exploring the current sector,
                    // branch off to neighboring sectors.
                    if start {
                        use crate::SectorDir::*;
                        // Direction list set up to do a boustrophedon on a
                        // big flat open sector-plane.
                        for sector_dir in [East, West, South, North, Down, Up] {
                            if let Some(dest) = loc
                                .path_dest_to_neighboring_sector(r, sector_dir)
                            {
                                if !r.autoexplore_map(dest).is_empty() {
                                    return self.decide(r, Goal::GoTo(dest));
                                }
                            }
                        }
                    }

                    // If not just starting autoexplore, stop here.
                    return None;
                }

                // After traveling to the alternate sector, bump out the
                // StartAutoexplore state here, should be followed by regular
                // Autoexplore via next_goal.
                if start {
                    return None;
                }

                if let Some(step) =
                    self.dijkstra_map_direction(r, &explore_map, loc)
                {
                    return Some(Action::Bump(step));
                } else {
                    return None;
                }
            }

            Goal::GoTo(loc) => {
                dest = loc;
            }

            Goal::AttackMove(loc) => {
                // Move towards actual target by default.
                dest = loc;

                // Look for targets of opportunity, redirect towards them.
                //
                // Mob will bump-to-attack the target.
                if let Some(e) = self.first_visible_enemy(r) {
                    if let Some(enemy_loc) = e.loc(r) {
                        dest = enemy_loc;
                    }
                }
            }

            Goal::Attack(e) => {
                if !e.is_alive(r) {
                    return None;
                }

                if let Some(loc) = e.loc(r) {
                    dest = loc;
                } else {
                    // Attack target can't be found, abandon goal.
                    return None;
                }
            }

            Goal::Escort(e) => {
                if !e.is_alive(r) {
                    return None;
                }

                if self.is_player(r) {
                    // Player mob should not have goals set that can make it
                    // stand around indefinitely. Currently the player mob
                    // rejects all escort goals.
                    return None;
                }
                if let Some(loc) = e.loc(r) {
                    dest = loc;
                } else {
                    // Follow target can't be found, abandon goal.
                    return None;
                }
            }
        }

        // We've got a pathfinding task from loc to dest.
        if loc == dest {
            // Drop out if already arrived.
            return None;
        }

        let enemy_at_dest =
            dest.mob_at(r).map_or(false, |e| e.is_enemy(r, self));

        if let Some(dir) = loc.vec_towards(&dest) {
            // Right next to the target. If it's a mob, we can attack.
            if dir.is_adjacent() {
                // Pointed towards an enemy, fight it.
                if enemy_at_dest {
                    return Some(Action::Bump(dir));
                }
                // Don't try to move into escorted mob's space.
                if matches!(goal, Goal::Escort(_)) {
                    return Some(Action::Pass);
                }
            }
        }

        // Path towards target.
        // Bit of difference, player-aligned mobs path according to seen
        // things, enemy mobs path according to full information.
        if let Some(mut path) = if self.is_player_aligned(r) {
            r.fov_aware_path_to(&loc, &dest)
        } else {
            r.path_to(&loc, &dest)
        } {
            // Path should always have a good step after a successful
            // pathfind.
            let next = path.pop().expect("Invalid pathfind: Empty path");

            // Path should be steppable.
            let dir = loc
                .find_step_towards(r, &next)
                .expect("Invalid pathfind: Not steppable");

            if self.can_step(r, dir) {
                return Some(Action::Bump(dir));
            }

            // Blocked by an undisplaceable mob, try to go around.
            let mut other_dirs: Vec<_> =
                s4::DIR.into_iter().filter(|&d| d != dir).collect();
            other_dirs.shuffle(&mut util::srng(&loc));
            for d in other_dirs {
                if self.can_step(r, d) {
                    return Some(Action::Bump(d));
                }
            }

            // Blocked by mobs, who will hopefully move out of the way.
            // Wait to pass the time.
            return Some(Action::Pass);
        }

        None
    }

    /// Figure out the next goal when current one is completed.
    pub fn next_goal(&self, r: &mut Runtime) {
        match self.goal(r) {
            Goal::None => {}
            Goal::FollowPlayer => {
                // Becomes invalid when you can't path to player.
                self.clear_goal(r);
            }
            Goal::StartAutoexplore => {
                self.set_goal(r, Goal::Autoexplore);
            }
            Goal::Autoexplore => {
                if self.is_npc(r) {
                    self.set_goal(r, Goal::FollowPlayer);
                } else {
                    self.clear_goal(r);
                }
            }
            Goal::GoTo(_) => {
                self.clear_goal(r);
            }
            Goal::AttackMove(_) => {
                if self.is_npc(r) {
                    self.set_goal(r, Goal::FollowPlayer);
                } else {
                    self.clear_goal(r);
                }
            }
            Goal::Attack(_) => {
                if self.is_npc(r) {
                    self.set_goal(r, Goal::FollowPlayer);
                } else {
                    self.clear_goal(r);
                }
            }
            Goal::Escort(_) => {
                if self.is_npc(r) {
                    self.set_goal(r, Goal::FollowPlayer);
                } else {
                    self.clear_goal(r);
                }
            }
        }
    }

    /// Project a ray in dir for n steps, try to find an enemy not blocked by
    /// walls.
    fn raycast_enemy(&self, r: &Runtime, dir: IVec2, n: i32) -> Option<Entity> {
        let loc = self.loc(r)?;
        for loc in loc.raycast(dir).take(n as usize) {
            if loc.tile(r).blocks_shot() {
                break;
            }

            if let Some(mob) = loc.mob_at(r) {
                if mob.is_enemy(r, self) {
                    return Some(mob);
                }
            }
        }

        None
    }

    pub fn target_for_attack(
        &self,
        r: &Runtime,
        dir: IVec2,
        weapon_slot: EquippedAt,
    ) -> Option<Entity> {
        let mut range = 1;
        if let Some(item) = self.equipment_at(r, weapon_slot) {
            if item.is_ranged_weapon(r) {
                // TODO Varying ranges for ranged weapons?
                range = THROW_DIST;
            }
        }

        self.raycast_enemy(r, dir, range)
    }

    /// Return current stats for an entity, factoring in its equipment.
    ///
    /// This method should always be used when querying the stats of a mob
    /// during gameplay, the raw `Stats` component has the base stats that
    /// don't include bonuses from equipment.
    pub fn stats(&self, r: &Runtime) -> Stats {
        let mut stats = self.get::<Stats>(r);
        for (_, e) in self.equipment(r) {
            stats += e.stats(r);
        }
        stats
    }

    /// Return score for how fast this entity should beat the other.
    #[allow(dead_code)]
    fn kill_speed(&self, r: &Runtime, other: &Entity) -> i32 {
        let s1 = self.stats(r);
        let s2 = other.stats(r);

        if s1.dmg == 0 {
            -1
        } else {
            (other.max_wounds(r) as f32 * Odds(s1.hit - s2.ev).prob()
                / s1.dmg as f32)
                .round() as i32
        }
    }

    pub fn goal(&self, r: &Runtime) -> Goal {
        self.get::<Goal>(r)
    }

    pub fn set_goal(&self, r: &mut Runtime, goal: Goal) {
        self.set(r, goal);
    }

    pub fn clear_goal(&self, r: &mut Runtime) {
        self.set(r, Goal::default());
    }

    pub(crate) fn is_looking_for_fight(&self, r: &Runtime) -> bool {
        matches!(self.goal(r), Goal::None | Goal::GoTo(_) | Goal::Escort(_))
            && Some(*self) != r.player()
    }

    pub(crate) fn fov_mobs(&self, r: &Runtime, range: i32) -> Vec<Entity> {
        // XXX: Not returning an iterator since I'm not bothering to handle
        // the nonexistent location component case into the chain.
        let Some(loc) = self.loc(r) else {
            return Default::default();
        };
        r.fov_from(loc, range)
            .filter_map(|(_, loc)| loc.mob_at(r))
            .collect()
    }

    /// Returns an enemy from the mob's FOV or `None` if there are no visible
    /// enemies.
    ///
    /// The selection criteria for the enemy should be that it's the most
    /// preferred target of opportunity for the current mob given its current
    /// FOV. The choice may depend on the capabilities of the queried mob.
    pub fn first_visible_enemy(&self, r: &Runtime) -> Option<Entity> {
        self.fov_mobs(r, FOV_RADIUS)
            .into_iter()
            .find(|e| e.is_enemy(r, self))
    }

    pub fn is_threatened(&self, r: &Runtime) -> bool {
        self.first_visible_enemy(r).is_some()
    }

    /// Return if there are threatening mobs in the given direction.
    pub fn is_threatened_from(&self, r: &Runtime, dir: IVec2) -> bool {
        debug_assert_eq!(s4::norm(dir), dir);
        self.fov_mobs(r, FOV_RADIUS)
            .into_iter()
            .filter(|&e| e.is_enemy(r, self))
            .any(|e| {
                if let Some(v) = self.vec_towards(r, &e) {
                    s4::norm(v) == dir
                } else {
                    false
                }
            })
    }

    pub(crate) fn scan_fov(&self, r: &mut Runtime) {
        let Some(loc) = self.loc(r) else { return };

        let cells: Vec<Location> =
            r.fov_from(loc, FOV_RADIUS).map(|(_, loc)| loc).collect();

        // Should we look for a fight while doing the scan?
        let mut looking_for_target = self.is_looking_for_fight(r);

        for loc in cells {
            if let Some(mob) = loc.mob_at(r) {
                if self.is_enemy(r, &mob) {
                    if looking_for_target {
                        // Found a target, go attack.
                        looking_for_target = false;
                        self.set_goal(r, Goal::Attack(mob));
                    }

                    // Alert the other mob to self.
                    if mob.alert_to(r, self) {
                        // Shout here if alert was successful, alert_to might
                        // get called from shout too. The first spotter is the
                        // one that makes noise.
                        mob.shout(r, Some(self));
                    }
                }
            }

            if self.is_player_aligned(r) {
                r.fov.insert(loc);
            }
        }
    }
}

/// Indirect long orders.
#[derive(
    Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize,
)]
pub enum Goal {
    #[default]
    None,

    /// Standing order for party member NPCs.
    ///
    /// Cannot be assigned to player.
    FollowPlayer,

    /// Special state at start of autoexploration
    ///
    /// If the current sector is fully explored and ongoing autoexploration
    /// would end, `StartAutoexplore` will instead look for unexplored
    /// adjacent sectors and plot a way to one if found.
    StartAutoexplore,

    /// Autoexplore the current sector.
    ///
    /// If given to player, will exit when enemies are sighted. If given to
    /// NPC, the NPC will fight sighted enemies. Completed when there are no
    /// reachable unexplored cells in the current sector.
    ///
    /// NPCs return to party when done.
    Autoexplore,

    /// Move to a location.
    ///
    /// NPCs will not resume following player when they arrive, used to
    /// detach NPCs from party.
    GoTo(Location),

    /// Like `GoTo`, but attack everything on the way.
    ///
    /// Unlike `GoTo`, NPCs will return to party once they arrive at the
    /// destination and see no targets of opportunity.
    AttackMove(Location),

    /// Attack a mob, will complete when target mob is dead.
    ///
    /// NPCs return to party when done.
    Attack(Entity),

    /// Escort target mob until it dies.
    ///
    /// NPCs return to party when done.
    ///
    /// Cannot be assigned to player. (Player should not get a goal that will
    /// cause the mob to stand around without working towards completing the
    /// goal.)
    Escort(Entity),
}

impl Goal {
    pub fn is_some(&self) -> bool {
        !matches!(self, Goal::None)
    }
}
