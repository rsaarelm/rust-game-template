//! Mobs figuring out what to do on their own.
use std::collections::BTreeSet;

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use util::{s4, Sdf};
use world::{Cube, EquippedAt};

use crate::{
    ecs::{IsEphemeral, IsFriendly},
    prelude::*,
    FOV_RADIUS, THROW_RANGE,
};

impl Entity {
    /// Decide on the next action given a goal.
    pub fn decide(
        &self,
        r: &impl AsRef<Runtime>,
        goal: Goal,
    ) -> Option<Action> {
        let r = r.as_ref();

        let loc = self.loc(r)?;
        let mut path_dest: Cube;

        match goal {
            Goal::None => return Some(Action::Pass),
            Goal::FollowPlayer => {
                let player = r.player()?;

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
                    path_dest = Cube::unit(enemy_loc);
                } else if let Some(loc) = player.loc(r) {
                    // Otherwise follow player.
                    path_dest = Cube::unit(loc);
                } else {
                    // Follow target can't be found, abandon goal.
                    return None;
                }
            }

            Goal::Autoexplore(zone) | Goal::StartAutoexplore(zone) => {
                let start = matches!(goal, Goal::StartAutoexplore(_));

                if !self.is_player(r) {
                    // Non-players fight when they run into enemies when
                    // exploring.
                    if let Some(e) = self.first_visible_enemy(r) {
                        return self.decide(r, Goal::Attack(e));
                    }
                }

                let explore_map =
                    r.autoexplore_map(&zone, loc, self.is_player(r));
                if explore_map.is_empty() {
                    return None;
                }

                // After traveling to the alternate sector, bump out the
                // StartAutoexplore state here, should be followed by regular
                // Autoexplore via next_goal.
                if start {
                    return None;
                }

                if let Some((n, step)) =
                    self.dijkstra_map_direction(r, &explore_map, loc)
                {
                    if n == 0 {
                        // We're hitting a target point in the map. Assume
                        // this is an autopickup item (the shroud edge targets
                        // recede as we approach them so they shouldn't be
                        // reachable) and use Bump action that picks things
                        // up.
                        return Some(Action::Bump(step));
                    } else {
                        // Otherwise use the non-picking-up step.
                        return Some(Action::Step(step));
                    }
                } else {
                    return None;
                }
            }

            Goal::GoTo {
                destination,
                is_attack_move,
                ..
            } => {
                path_dest = destination;

                // Look for targets of opportunity, redirect towards them.
                //
                // Mob will bump-to-attack the target.
                if is_attack_move {
                    if let Some(e) = self.first_visible_enemy(r) {
                        if let Some(enemy_loc) = e.loc(r) {
                            path_dest = Cube::unit(enemy_loc);
                        }
                    }
                }
            }

            Goal::Attack(e) => {
                if !e.is_alive(r) {
                    return None;
                }

                if let Some(loc) = e.loc(r) {
                    path_dest = Cube::unit(loc);
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
                    path_dest = Cube::unit(loc);
                } else {
                    // Follow target can't be found, abandon goal.
                    return None;
                }
            }
        }

        // We've got a pathfinding task from loc to dest.
        if path_dest.sd(loc) <= 0 {
            // Drop out if already arrived.
            return None;
        }

        // Right next to dest, see if we need to watch out for mobs.
        if let Some((dir, dest)) = loc
            .walk_neighbors(r)
            .find(|(_, loc)| path_dest.sd(*loc) <= 0)
        {
            // There's an enemy, fight it.
            if dest.mob_at(r).map_or(false, |e| e.is_enemy(r, self)) {
                return Some(Action::Bump(dir));
            }

            // Don't try to move into escorted mob's space.
            if matches!(goal, Goal::Escort(_)) {
                return Some(Action::Pass);
            }

            // Finish with a bump action to pick up items.
            if self.can_step(r, dir) {
                return Some(Action::Bump(dir));
            }
        }

        // Path towards target.
        // Bit of difference, player-aligned mobs path according to seen
        // things, enemy mobs path according to full information.
        if let Some(mut path) = {
            if self.is_player_aligned(r) {
                // Try to path through only known areas first, then by
                // exploring
                r.find_path(FogPathing::Avoid, loc, &path_dest).or_else(|| {
                    r.find_path(FogPathing::Explore, loc, &path_dest)
                })
            } else {
                r.find_path(FogPathing::Ignore, loc, &path_dest)
            }
        } {
            // Path should always have a good step after a successful
            // pathfind.
            let next = path.pop().expect("Invalid pathfind: Empty path");

            // Path should be steppable.
            let dir = loc
                .vec2_towards(&next)
                .expect("Invalid pathfind: Not steppable");
            assert_eq!(
                dir.length_squared(),
                1,
                "Invalid pathfind: Bad step distance"
            );

            if self.can_step(r, dir) {
                return Some(Action::Step(dir));
            }

            // Blocked by an undisplaceable mob, try to go around.
            let mut other_dirs: Vec<_> =
                s4::DIR.into_iter().filter(|&d| d != dir).collect();
            other_dirs.shuffle(&mut util::srng(&loc));
            for d in other_dirs {
                if self.can_step(r, d) {
                    return Some(Action::Step(d));
                }
            }

            // Blocked by mobs, who will hopefully move out of the way.
            // Wait to pass the time.
            return Some(Action::Pass);
        }

        None
    }

    /// Figure out the next goal when current one is completed.
    pub fn next_goal(&self, r: &mut impl AsMut<Runtime>) {
        let r = r.as_mut();

        match self.goal(r) {
            Goal::None => {}
            Goal::FollowPlayer => {
                // Becomes invalid when you can't path to player.
                self.clear_goal(r);
            }
            Goal::StartAutoexplore(zone) => {
                self.set_goal(r, Goal::Autoexplore(zone));
            }
            Goal::Autoexplore(_) => {
                if self.is_npc(r) {
                    self.set_goal(r, Goal::FollowPlayer);
                } else {
                    self.clear_goal(r);
                }
            }
            Goal::GoTo { is_attack_move, .. } => {
                if is_attack_move && self.is_npc(r) {
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

    pub fn is_threatened(&self, r: &impl AsRef<Runtime>) -> bool {
        self.first_visible_enemy(r).is_some()
    }

    /// Return if there are threatening mobs in the given direction.
    pub fn is_threatened_from(
        &self,
        r: &impl AsRef<Runtime>,
        dir: IVec2,
    ) -> bool {
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

    pub fn is_player_aligned(&self, r: &impl AsRef<Runtime>) -> bool {
        self.get::<IsFriendly>(r).0
    }

    pub fn can_become_player(&self, r: &impl AsRef<Runtime>) -> bool {
        // Allied
        self.is_player_aligned(r) && !self.get::<IsEphemeral>(r).0
    }

    pub fn is_enemy(&self, r: &impl AsRef<Runtime>, other: &Entity) -> bool {
        self.is_player_aligned(r) != other.is_player_aligned(r)
    }

    pub fn is_ally(&self, r: &impl AsRef<Runtime>, other: &Entity) -> bool {
        self.is_player_aligned(r) == other.is_player_aligned(r)
    }

    pub fn goal(&self, r: &impl AsRef<Runtime>) -> Goal {
        self.get::<Goal>(r)
    }

    pub fn set_goal(&self, r: &mut impl AsMut<Runtime>, goal: Goal) {
        self.set(r, goal);
    }

    pub fn order_go_to(&self, r: &mut impl AsMut<Runtime>, loc: Location) {
        let r = r.as_mut();
        let Some(origin) = self.loc(r) else { return };
        let is_exploring = self.is_player_aligned(r) && !loc.is_explored(r);
        self.set_goal(
            r,
            Goal::GoTo {
                origin,
                destination: Cube::unit(loc),
                is_attack_move: false,
                is_exploring,
            },
        )
    }

    pub fn order_go_to_zone(&self, r: &mut impl AsMut<Runtime>, zone: Cube) {
        let r = r.as_mut();
        let Some(origin) = self.loc(r) else { return };
        self.set_goal(
            r,
            Goal::GoTo {
                origin,
                destination: zone,
                is_attack_move: false,
                is_exploring: false,
            },
        )
    }

    pub fn order_attack_move(
        &self,
        r: &mut impl AsMut<Runtime>,
        loc: Location,
    ) {
        let r = r.as_mut();
        let Some(origin) = self.loc(r) else { return };
        self.set_goal(
            r,
            Goal::GoTo {
                origin,
                destination: Cube::unit(loc),
                is_attack_move: true,
                is_exploring: false,
            },
        )
    }

    pub fn clear_goal(&self, r: &mut impl AsMut<Runtime>) {
        self.set(r, Goal::default());
    }

    /// If mob is player and doing an autopiloted long move, stop.
    pub fn stop_player_autopilot(&self, r: &mut impl AsMut<Runtime>) {
        let r = r.as_mut();

        if !self.is_player(r) {
            return;
        }

        self.clear_goal(r);
    }

    pub(crate) fn target_for_attack(
        &self,
        r: &impl AsRef<Runtime>,
        dir: IVec2,
        weapon_slot: EquippedAt,
    ) -> Option<Entity> {
        let r = r.as_ref();
        let mut range = 1;
        if let Some(item) = self.equipment_at(r, weapon_slot) {
            if item.is_ranged_weapon(r) {
                // TODO Varying ranges for ranged weapons?
                range = THROW_RANGE as usize;
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

    pub(crate) fn is_looking_for_fight(&self, r: &impl AsRef<Runtime>) -> bool {
        let r = r.as_ref();
        matches!(
            self.goal(r),
            Goal::None | Goal::GoTo { .. } | Goal::Escort(_)
        ) && Some(*self) != r.player()
    }

    pub(crate) fn fov_mobs(
        &self,
        r: &impl AsRef<Runtime>,
        range: i32,
    ) -> Vec<Entity> {
        let r = r.as_ref();
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
    pub fn first_visible_enemy(
        &self,
        r: &impl AsRef<Runtime>,
    ) -> Option<Entity> {
        self.fov_mobs(r, FOV_RADIUS)
            .into_iter()
            .find(|e| e.is_enemy(r, self))
    }

    pub(crate) fn scan_fov(&self, r: &mut impl AsMut<Runtime>) {
        let r = r.as_mut();

        if !self.is_mob(r) {
            return;
        }

        let Some(loc) = self.loc(r) else { return };

        let cells: Vec<Location> =
            r.fov_from(loc, FOV_RADIUS).map(|(_, loc)| loc).collect();

        // Should we look for a fight while doing the scan?
        let mut looking_for_target = self.is_looking_for_fight(r);

        let mut revealed = Vec::new();

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

            if self.is_player_aligned(r) && !r.fov.contains(&loc) {
                revealed.push(loc);
                r.fov.insert(loc);
            }
        }

        // See what to do with new stuff that was revealed.

        // Collect stuff into intermediate containers so that similar things
        // will get grouped together in display.
        let mut items = BTreeSet::default();
        let mut mobs = BTreeSet::default();
        for loc in &revealed {
            // If found an item, emit a message "You found (item name)." using
            // the templating system and stop the mob's autoexplore if it's
            // autoexploring.
            if let Some(item) = loc.item_at(r) {
                items.insert(item.noun(r));
                self.stop_player_autopilot(r);
            }

            if let Some(creature) = loc.mob_at(r) {
                if self.is_enemy(r, &creature) {
                    mobs.insert(creature.noun(r));
                    self.stop_player_autopilot(r);
                }
            }
        }

        for n in mobs {
            msg!("[One] spot[s] [a thing]."; self.noun(r), n);
        }

        for n in items {
            msg!("[One] found [a thing]."; self.noun(r), n);
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
    StartAutoexplore(Cube),

    /// Autoexplore the current sector.
    ///
    /// If given to player, will exit when enemies are sighted. If given to
    /// NPC, the NPC will fight sighted enemies. Completed when there are no
    /// reachable unexplored cells in the current sector.
    ///
    /// NPCs return to party when done.
    Autoexplore(Cube),

    /// Move to a location.
    ///
    /// Option to attack anything encountered for player's NPCs.
    GoTo {
        /// Keep track of starting point to limit pathfinding queries.
        // TODO: Do we still need the GoTo::origin field, current pathfinding logic
        // shouldn't be able to stray out of the starting volume...
        origin: Location,
        /// Destination is a zone, use an unit cube for a point.
        destination: Cube,
        /// If true, NPC will look for fights along the way.
        is_attack_move: bool,
        /// If true, move is into unexplored terrain.
        is_exploring: bool,
    },

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
