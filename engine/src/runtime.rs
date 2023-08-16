use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use util::{flood_fill_4, GameRng};

use crate::{ecs::*, prelude::*, Fov, Placement, Result, Terrain, World};

/// Game runtime main engine data container.
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Runtime {
    now: Instant,
    pub(crate) player: Option<Entity>,
    pub(crate) terrain: Terrain,
    pub(crate) fov: Fov,
    pub(crate) ecs: Ecs,
    pub(crate) placement: Placement,
    pub(crate) rng: GameRng,
}

impl Default for Runtime {
    fn default() -> Self {
        Runtime {
            now: Default::default(),
            rng: GameRng::seed_from_u64(0xdeadbeef),
            player: Default::default(),
            terrain: Default::default(),
            fov: Default::default(),
            ecs: Default::default(),
            placement: Default::default(),
        }
    }
}

impl Runtime {
    pub fn new(w: &World) -> Result<Self> {
        let mut ret = Runtime::default();

        // Start placing spawns when the world is finished so they can react
        // to the terrain properly (eg. for initial FOV calculation).
        let mut spawns = Vec::new();

        for (loc, c) in w.terrain.iter() {
            if c == '@' {
                ret.terrain.insert(loc, Tile::Ground);
                if ret.player.is_some() {
                    // Ignore subsequent player entrances.
                    // Could do stuff with them like collecting all and
                    // randomly choosing between one.
                    continue;
                }
                // Player entrance
                // TODO Bring player data in from outside Worldfile in API
                let player = Entity(ret.ecs.spawn((
                    Name("Player".into()),
                    Icon('H'),
                    Speed(4),
                    Level(5),
                    IsMob(true),
                    IsFriendly(true),
                    Stats {
                        hit: 4,
                        ev: 2,
                        dmg: 4,
                    },
                )));
                spawns.push((player, loc));
                ret.player = Some(player);
            } else if let Some(spawn) = w.legend.get(&c) {
                // Entity spawn.
                let Some(prototype) = w.lexicon.get(spawn) else {
                    return err(format!("Bad spawn {spawn:?}"));
                };

                let e = ret.spawn(prototype);
                spawns.push((e, loc));

                // Create a sensible terrain type under it.
                let t = e.preferred_tile(&ret);
                ret.terrain.insert(loc, t);
            } else if let Ok(t) = Tile::try_from(c) {
                ret.terrain.insert(loc, t);
            } else {
                return err(format!("Bad worldfile tile {c:?}"));
            }
        }

        if ret.player.is_none() {
            return err("Worldfile does not specify player entry point");
        }

        for (e, loc) in spawns {
            e.place(&mut ret, loc);
        }

        // Start time from an above-zero value so that zero time values can
        // work as "unspecified time".
        ret.now = Instant(3600);

        Ok(ret)
    }

    pub fn now(&self) -> Instant {
        self.now
    }

    /// Access the persistent engine random number generator.
    pub(crate) fn rng(&mut self) -> &mut impl rand::Rng {
        &mut self.rng
    }

    /// Remove dead entities from ECS.
    pub(crate) fn gc(&mut self) {
        let kill_list: Vec<Entity> =
            self.ecs.iter().filter(|e| !e.is_alive(self)).collect();
        for e in kill_list {
            self.ecs.0.despawn(e.0).expect("Bad entity ID");
        }
    }

    pub fn spawn(&mut self, loadout: impl hecs::DynamicBundle) -> Entity {
        Entity(self.ecs.spawn(loadout))
    }

    pub fn player(&self) -> Option<Entity> {
        self.player
    }

    pub fn live_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.placement.all_entities()
    }

    /// Update the crate state by one second.
    pub fn update(&mut self) {
        // Tick every entity every frame
        let all: Vec<Entity> = self.live_entities().collect();
        for e in all {
            e.tick(self);
        }

        // Collect entities that can act this frame.
        let mut actives: Vec<Entity> = self
            .live_entities()
            .filter(|e| e.acts_this_frame(self))
            .collect();

        while let Some(e) = actives.pop() {
            // Discard dead entities, they might have died during the update
            // loop.
            if !e.is_alive(self) {
                continue;
            }

            let goal = e.goal(self);
            if goal != Goal::None {
                if e.is_player(self) && e.first_visible_enemy(self).is_some() {
                    // Abort commands when player is threatened. (This is
                    // placed here instead of inside `decide` so that the
                    // player can still be made to single-step the goal by
                    // calling decide when under threat.)
                    match goal {
                        Goal::GoTo(_) => e.next_goal(self),
                        Goal::Autoexplore => e.next_goal(self),
                        _ => {}
                    }
                }

                if let Some(act) = e.decide(self, goal) {
                    e.execute(self, act);
                } else {
                    e.next_goal(self);
                }
            }
        }

        self.now += 1;
        self.gc();
    }

    pub fn autoexplore_map(&self, loc: Location) -> HashMap<Location, usize> {
        let ret: HashMap<Location, usize> = flood_fill_4(
            &|loc2: &Location| {
                loc2.sector() == loc.sector() && loc2.is_walkable(self)
            },
            loc.expanded_sector_locs()
                .filter(|loc| !loc.is_explored(self)),
        )
        .collect();

        if !ret.contains_key(&loc) {
            // Map must reach the starting location.
            Default::default()
        } else {
            ret
        }
    }

    /// A fill-positions variant that assumes all FOV-covered cells are passable.
    pub fn fov_optimistic_fill_positions(
        &self,
        start: Location,
    ) -> impl Iterator<Item = Location> + '_ {
        util::dijkstra_map(
            move |&loc| {
                loc.neighbors_4()
                    .into_iter()
                    .filter(|loc2| {
                        (!loc2.is_explored(self) || loc2.is_walkable(self))
                            && loc2.sector() == loc.sector()
                    })
                    .collect::<Vec<Location>>()
            },
            [start],
        )
        .map(|n| n.0)
    }

    pub fn fill_positions(
        &self,
        start: Location,
    ) -> impl Iterator<Item = Location> + '_ {
        util::dijkstra_map(
            move |&loc| {
                loc.neighbors_4()
                    .into_iter()
                    .filter(|loc2| {
                        loc2.is_walkable(self) && loc2.sector() == loc.sector()
                    })
                    .collect::<Vec<Location>>()
            },
            [start],
        )
        .map(|n| n.0)
    }

    /// Start filling positions around given location while staying within
    /// the same sector and on walkable tiles.
    pub fn perturbed_fill_positions(
        &self,
        start: Location,
    ) -> impl Iterator<Item = Location> + '_ {
        util::dijkstra_map(
            move |&loc| {
                loc.perturbed_neighbors_4()
                    .into_iter()
                    .filter(|loc2| {
                        loc2.is_walkable(self) && loc2.sector() == loc.sector()
                    })
                    .collect::<Vec<Location>>()
            },
            [start],
        )
        .map(|n| n.0)
    }

    pub fn path_to(
        &self,
        start: &Location,
        dest: &Location,
    ) -> Option<Vec<Location>> {
        let dest = dest.fold(self);

        // Bail out early if it looks like we need more than one sector
        // transition.
        if start.sector_dist(&dest) > 1 {
            return None;
        }

        util::astar_path(
            start,
            &dest,
            |&loc| {
                loc.fold_neighbors_4(self).filter(|loc| {
                    start.sector_dist(loc) <= 1 && loc.is_walkable(self)
                })
            },
            Location::astar_heuristic,
        )
    }

    /// Plan a path without information not revealed in player's FOV.
    ///
    /// Optimistically expect all unexplored cells to be traversable.
    pub fn fov_aware_path_to(
        &self,
        start: &Location,
        dest: &Location,
    ) -> Option<Vec<Location>> {
        let dest = dest.fold(self);

        if start.sector_dist(&dest) > 1 {
            return None;
        }

        util::astar_path(
            start,
            &dest,
            |&loc| {
                loc.fold_neighbors_4(self).filter(|loc| {
                    // Optimistically run into the fog of war within the
                    // starting sector.
                    (loc.sector() == start.sector() && !loc.is_explored(self))
                    // Allow hitting the end loc even if it's not visible
                    // so you can path into unexplored stairwells.
                        || (loc == &dest && !loc.is_explored(self))
                    // Otherwise search within 1 sector transition of
                    // current loc only across known-to-be-passable locs.
                        || (start.sector_dist(loc) <= 1
                            && loc.is_explored(self)
                            && loc.is_walkable(self))
                })
            },
            Location::astar_heuristic,
        )
    }

    /// Return whether the overall game scenario is still going or if it has
    /// ended in victory or defeat.
    pub fn scenario_status(&self) -> ScenarioStatus {
        if self.player().is_none() {
            return ScenarioStatus::Lost;
        }

        // TODO win condition
        ScenarioStatus::Ongoing
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn simple_game() {
        let w: crate::World = idm::from_str(
            "\
:terrain
  0 0 0
    ####  ####
    #..####..#
    #.@......#
    ##......##
     #...a..#
     #......#
    ##......##
    #........#
    #..####..#
    ####  ####
:legend
  a critter
:lexicon
  critter
    :icon c
    :speed 4
    :stats 3 2 4
    :level 4",
        )
        .unwrap();
        let mut c = Runtime::new(&w).unwrap();

        let player = c.player().expect("No player entity created");
        // Set field to value.
        player.set(&mut c, Level(8));
        // Set field to default, this should remove the component.
        player.set(&mut c, Momentum(Default::default()));

        let save1 = idm::to_string(&c).unwrap();
        eprintln!("{save1}");

        let c2: Runtime = idm::from_str(&save1).unwrap();
        assert_eq!(c.placement, c2.placement);

        let save2 = idm::to_string(&c2).unwrap();
        assert_eq!(save1, save2);
    }
}
