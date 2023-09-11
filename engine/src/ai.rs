//! Mobs figuring out what to do on their own.
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use util::s4;

use crate::{ecs::IsFriendly, prelude::*, EquippedAt, FOV_RADIUS, THROW_DIST};

impl Entity {
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

    pub fn is_player_aligned(&self, r: &Runtime) -> bool {
        self.get::<IsFriendly>(r).0
    }

    pub fn is_enemy(&self, r: &Runtime, other: &Entity) -> bool {
        self.is_player_aligned(r) != other.is_player_aligned(r)
    }

    pub fn is_ally(&self, r: &Runtime, other: &Entity) -> bool {
        self.is_player_aligned(r) == other.is_player_aligned(r)
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

    pub(crate) fn target_for_attack(
        &self,
        r: &Runtime,
        dir: IVec2,
        weapon_slot: EquippedAt,
    ) -> Option<Entity> {
        let mut range = 1;
        if let Some(item) = self.equipment_at(r, weapon_slot) {
            if item.is_ranged_weapon(r) {
                // TODO Varying ranges for ranged weapons?
                range = THROW_DIST as usize;
            }
        }

        if let Some(loc) = self.loc(r) {
            // If you're confused, stop avoiding friendly fire.
            let perp = if self.is_confused(r) {
                None
            } else {
                Some(*self)
            };

            if let Some(enemy) = r.trace_enemy(perp, loc, dir, range) {
                return Some(enemy);
            }
        }
        None
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
