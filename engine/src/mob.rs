//! Entity logic for active creatures.
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::{
    ecs::{ActsNext, IsFriendly, IsMob, Momentum, Speed, Stats},
    prelude::*,
    EquippedAt, FOV_RADIUS, PHASES_IN_TURN, THROW_DIST,
};

impl Entity {
    pub fn is_player(&self, c: &Core) -> bool {
        c.player == Some(*self)
    }

    pub fn become_player(&self, c: &mut Core) {
        let prev_player = c.player();

        c.player = Some(*self);
        // Clear goal, existing ones probably won't make sense for a
        // player mob.
        self.clear_goal(c);

        if let Some(prev_player) = prev_player {
            // Give the previous player mob a follower AI so it won't just
            // stand around.
            if prev_player != *self {
                prev_player.set_goal(c, Goal::FollowPlayer);
            }
        }
    }

    pub fn is_mob(&self, c: &Core) -> bool {
        self.get::<IsMob>(c).0
    }

    pub fn is_player_aligned(&self, c: &Core) -> bool {
        self.get::<IsFriendly>(c).0
    }

    pub fn is_npc(&self, c: &Core) -> bool {
        self.is_player_aligned(c) && !self.is_player(c)
    }

    pub fn is_enemy(&self, c: &Core, other: &Entity) -> bool {
        self.is_player_aligned(c) != other.is_player_aligned(c)
    }

    pub fn is_ally(&self, c: &Core, other: &Entity) -> bool {
        self.is_player_aligned(c) == other.is_player_aligned(c)
    }

    /// Return the vector the mob is moving along "right now".
    ///
    /// Used for displace deadlocks, a mob with existing momentum shouldn't be
    /// displaced against the momentum vector.
    pub fn live_momentum(&self, c: &Core) -> IVec2 {
        if self.acts_next(c) > c.now() {
            // Only display momentum if the mob has moved during the current
            // phase.
            self.get::<Momentum>(c).0
        } else {
            Default::default()
        }
    }

    pub fn can_step(&self, c: &Core, dir: IVec2) -> bool {
        let Some(loc) = self.loc(c) else { return false };
        let n = (loc + dir).fold(c);

        if !n.is_walkable(c) {
            return false;
        }

        if let Some(mob) = n.mob_at(c) {
            if !self.can_displace(c, dir, &mob) {
                return false;
            }
        }

        true
    }

    pub fn can_displace(&self, c: &Core, dir: IVec2, other: &Entity) -> bool {
        if other.is_player(c) {
            // Don't displace the player.
            return false;
        }

        if !self.is_ally(c, other) {
            // Can't displace enemies.
            return false;
        }

        if self.is_player(c) {
            // Player can displace regardless of momentum.
            return true;
        }

        let m = other.live_momentum(c);
        if (m - dir).taxi_len() < m.taxi_len() {
            // Never displace against live momentum, this avoids deadlocks
            // where two NPCs try move in opposing directions and keep
            // displacing each other.
            return false;
        }

        true
    }

    pub fn acts_next(&self, c: &Core) -> Instant {
        self.get::<ActsNext>(c).0
    }

    pub fn acts_this_frame(&self, c: &Core) -> bool {
        self.get::<Speed>(c).0 > 0 && self.acts_next(c) <= c.now()
    }

    pub fn acts_before_next_player_frame(&self, c: &Core) -> bool {
        if let Some(player) = c.player() {
            self.acts_next(c) <= player.next_phase_frame(c)
        } else {
            self.acts_this_frame(c)
        }
    }

    pub fn can_be_commanded(&self, c: &Core) -> bool {
        // NPCs can be commanded up to one full turn into the future.
        self.is_alive(c) && self.acts_next(c).elapsed(c) < PHASES_IN_TURN
    }

    /// Special method to immediately run goals on a NPC.
    ///
    /// Returns false if there is no goal or the NPC can't be commanded
    /// anymore on this turn.
    ///
    /// NB. Since this is meant for running explicit orders on NPCs, it does
    /// nothing and returns false if the goal is the default `FollowPlayer`.
    pub fn exhaust_actions(&self, c: &mut Core) {
        if !self.is_npc(c) {
            return;
        }

        // XXX: Goals might spin wheels forever, so add a release valve of
        // only spinning for a limited number of rounds.
        for _failsafe in 0..32 {
            let goal = self.goal(c);
            if !self.can_be_commanded(c) {
                break;
            }

            if matches!(goal, Goal::None | Goal::FollowPlayer) {
                break;
            }

            if let Some(act) = self.decide(c, goal) {
                self.execute(c, act);
            } else {
                self.next_goal(c);
                break;
            }
        }
    }

    pub(crate) fn next_phase_frame(&self, c: &Core) -> Instant {
        let mut t = self.acts_next(c).max(c.now()) + 1;
        let speed = self.get::<Speed>(c).0;
        assert!(speed > 0);

        while !t.is_action_frame(speed) {
            t += 1;
        }
        t
    }

    /// Decide on the next action given a goal.
    pub fn decide(&self, c: &Core, goal: Goal) -> Option<Action> {
        let mut dest;

        let Some(loc) = self.loc(c) else { return None };

        match goal {
            Goal::None => return Some(Action::Pass),
            Goal::FollowPlayer => {
                let Some(player) = c.player() else {
                    return None;
                };

                if self.is_player(c) {
                    log::warn!(
                        "mob::decide: Player was assigned FollowPlayer goal"
                    );
                    return None;
                }

                if let Some(enemy_loc) =
                    self.first_visible_enemy(c).and_then(|e| e.loc(c))
                {
                    // Enemies visible! Go fight them.
                    dest = enemy_loc;
                } else if let Some(loc) = player.loc(c) {
                    // Otherwise follow player.
                    dest = loc;
                } else {
                    // Follow target can't be found, abandon goal.
                    return None;
                }
            }

            Goal::Autoexplore | Goal::StartAutoexplore => {
                let start = matches!(goal, Goal::StartAutoexplore);

                if let Some(e) = self.first_visible_enemy(c) {
                    // Fight when enemies sighted.
                    return self.decide(c, Goal::Attack(e));
                }

                let explore_map = c.autoexplore_map(loc);
                if explore_map.is_empty() {
                    // If starting out and done exploring the current sector,
                    // branch off to neighboring sectors.
                    if start {
                        use crate::SectorDir::*;
                        for sector_dir in [East, West, South, North, Down, Up] {
                            if let Some(dest) = loc
                                .path_dest_to_neighboring_sector(c, sector_dir)
                            {
                                if !c.autoexplore_map(dest).is_empty() {
                                    return self.decide(c, Goal::GoTo(dest));
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
                    self.dijkstra_map_direction(c, &explore_map, loc)
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
                if let Some(e) = self.first_visible_enemy(c) {
                    if let Some(enemy_loc) = e.loc(c) {
                        dest = enemy_loc;
                    }
                }
            }

            Goal::Attack(e) => {
                if !e.is_alive(c) {
                    return None;
                }

                if let Some(loc) = e.loc(c) {
                    dest = loc;
                } else {
                    // Attack target can't be found, abandon goal.
                    return None;
                }
            }

            Goal::Escort(e) => {
                if !e.is_alive(c) {
                    return None;
                }

                if self.is_player(c) {
                    // Player mob should not have goals set that can make it
                    // stand around indefinitely. Currently the player mob
                    // rejects all escort goals.
                    return None;
                }
                if let Some(loc) = e.loc(c) {
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
            dest.mob_at(c).map_or(false, |e| e.is_enemy(c, self));

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
        if let Some(mut path) = if self.is_player_aligned(c) {
            c.fov_aware_path_to(&loc, &dest)
        } else {
            c.path_to(&loc, &dest)
        } {
            // Path should always have a good step after a successful
            // pathfind.
            let next = path.pop().expect("Invalid pathfind: Empty path");

            // Path should be steppable.
            let dir = loc
                .find_step_towards(c, &next)
                .expect("Invalid pathfind: Not steppable");

            if self.can_step(c, dir) {
                return Some(Action::Bump(dir));
            }

            // Blocked by an undisplaceable mob, try to go around.
            let mut other_dirs: Vec<_> =
                DIR_4.into_iter().filter(|&d| d != dir).collect();
            other_dirs.shuffle(&mut util::srng(&loc));
            for d in other_dirs {
                if self.can_step(c, d) {
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
    pub fn next_goal(&self, c: &mut Core) {
        match self.goal(c) {
            Goal::None => {}
            Goal::FollowPlayer => {
                // Becomes invalid when you can't path to player.
                self.clear_goal(c);
            }
            Goal::StartAutoexplore => {
                self.set_goal(c, Goal::Autoexplore);
            }
            Goal::Autoexplore => {
                if self.is_npc(c) {
                    self.set_goal(c, Goal::FollowPlayer);
                } else {
                    self.clear_goal(c);
                }
            }
            Goal::GoTo(_) => {
                self.clear_goal(c);
            }
            Goal::AttackMove(_) => {
                if self.is_npc(c) {
                    self.set_goal(c, Goal::FollowPlayer);
                } else {
                    self.clear_goal(c);
                }
            }
            Goal::Attack(_) => {
                if self.is_npc(c) {
                    self.set_goal(c, Goal::FollowPlayer);
                } else {
                    self.clear_goal(c);
                }
            }
            Goal::Escort(_) => {
                if self.is_npc(c) {
                    self.set_goal(c, Goal::FollowPlayer);
                } else {
                    self.clear_goal(c);
                }
            }
        }
    }

    /// Project a ray in dir for n steps, try to find an enemy not blocked by
    /// walls.
    fn raycast_enemy(&self, c: &Core, dir: IVec2, n: i32) -> Option<Entity> {
        let loc = self.loc(c)?;
        for loc in loc.raycast(dir).take(n as usize) {
            if loc.tile(c).blocks_shot() {
                break;
            }

            if let Some(mob) = loc.mob_at(c) {
                if mob.is_enemy(c, self) {
                    return Some(mob);
                }
            }
        }

        None
    }

    pub fn attack_target(
        &self,
        c: &Core,
        dir: IVec2,
        weapon_slot: EquippedAt,
    ) -> Option<Entity> {
        let mut range = 1;
        if let Some(item) = self.equipment_at(c, weapon_slot) {
            if item.is_ranged_weapon(c) {
                // TODO Varying ranges for ranged weapons?
                range = THROW_DIST;
            }
        }

        self.raycast_enemy(c, dir, range)
    }

    /// Return current stats for an entity, factoring in its equipment.
    ///
    /// This method should always be used when querying the stats of a mob
    /// during gameplay, the raw `Stats` component has the base stats that
    /// don't include bonuses from equipment.
    pub fn stats(&self, c: &Core) -> Stats {
        let mut stats = self.get::<Stats>(c);
        for (_, e) in self.current_equipment(c) {
            stats += e.stats(c);
        }
        stats
    }

    /// Return score for how fast this entity should beat the other.
    #[allow(dead_code)]
    fn kill_speed(&self, c: &Core, other: &Entity) -> i32 {
        let s1 = self.stats(c);
        let s2 = other.stats(c);

        if s1.dmg == 0 {
            return -1;
        } else {
            (other.max_wounds(c) as f32 * Odds(s1.hit - s2.ev).prob()
                / s1.dmg as f32)
                .round() as i32
        }
    }

    pub fn goal(&self, c: &Core) -> Goal {
        self.get::<Goal>(c)
    }

    pub fn set_goal(&self, c: &mut Core, goal: Goal) {
        self.set(c, goal);
    }

    pub fn clear_goal(&self, c: &mut Core) {
        self.set(c, Goal::default());
    }

    pub(crate) fn is_looking_for_fight(&self, c: &Core) -> bool {
        matches!(self.goal(c), Goal::None | Goal::GoTo(_) | Goal::Escort(_))
            && Some(*self) != c.player()
    }

    pub(crate) fn fov_mobs(&self, c: &Core, range: i32) -> Vec<Entity> {
        // XXX: Not returning an iterator since I'm not bothering to handle
        // the nonexistent location component case into the chain.
        let Some(loc) = self.loc(c) else {
            return Default::default();
        };
        c.fov_from(loc, range)
            .filter_map(|(_, loc)| loc.mob_at(c))
            .collect()
    }

    /// Returns an enemy from the mob's FOV or `None` if there are no visible
    /// enemies.
    ///
    /// The selection criteria for the enemy should be that it's the most
    /// preferred target of opportunity for the current mob given its current
    /// FOV. The choice may depend on the capabilities of the queried mob.
    pub fn first_visible_enemy(&self, c: &Core) -> Option<Entity> {
        self.fov_mobs(c, FOV_RADIUS)
            .into_iter()
            .filter(|e| e.is_enemy(c, self))
            .next()
    }

    pub(crate) fn scan_fov(&self, c: &mut Core) {
        let Some(loc) = self.loc(c) else { return };

        let cells: Vec<Location> =
            c.fov_from(loc, FOV_RADIUS).map(|(_, loc)| loc).collect();

        // Should we look for a fight while doing the scan?
        let mut looking_for_target = self.is_looking_for_fight(c);

        for loc in cells {
            if let Some(mob) = loc.mob_at(c) {
                if self.is_enemy(c, &mob) {
                    if looking_for_target {
                        // Found a target, go attack.
                        looking_for_target = false;
                        self.set_goal(c, Goal::Attack(mob));
                    }

                    // Alert the other mob to self.
                    if mob.alert_to(c, self) {
                        // Shout here if alert was successful, alert_to might
                        // get called from shout too. The first spotter is the
                        // one that makes noise.
                        mob.shout(c, Some(self));
                    }
                }
            }

            if self.is_player_aligned(c) {
                c.fov.insert(loc);
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
    /// Will go through some conditions that would cause an ongoing
    /// autoexplore to halt.
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
