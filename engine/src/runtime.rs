use anyhow::Result;
use content::{Data, World};
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use util::{flood_fill_4, s8, GameRng, Logos};

use crate::{ecs::*, placement::Place, prelude::*, EntitySpec, Fov, Placement};

/// Main data container for game engine runtime.
#[derive(Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Runtime {
    now: Instant,
    pub(crate) player: Option<Entity>,
    pub(crate) fov: Fov,
    pub(crate) ecs: Ecs,
    pub(crate) placement: Placement,
    pub(crate) rng: GameRng,
    pub(crate) world: World,
}

impl AsRef<Runtime> for Runtime {
    fn as_ref(&self) -> &Runtime {
        self
    }
}

impl AsMut<Runtime> for Runtime {
    fn as_mut(&mut self) -> &mut Runtime {
        self
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Runtime {
            // Start time from an above-zero value so that zero time values
            // can work as "unspecified time".
            now: Instant(3600),
            rng: GameRng::seed_from_u64(0xdeadbeef),
            player: Default::default(),
            fov: Default::default(),
            ecs: Default::default(),
            placement: Default::default(),
            world: Default::default(),
        }
    }
}

impl Runtime {
    pub fn new(seed: Logos) -> Result<Self> {
        let world = World::new(seed, Data::get().scenario.clone())?;
        let rng = util::srng(world.seed());

        let mut ret = Runtime {
            world,
            rng,
            ..Default::default()
        };

        // Construct the initial world space and create the spawns.
        ret.refresh_world_cache(ret.world.player_entrance().into());

        ret.spawn_player_at(ret.world.player_entrance().into());

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

    pub fn spawn(&mut self, spawn: &content::Spawn) -> Entity {
        match spawn {
            content::Spawn::Monster(name, data) => data.build(self, name),
            content::Spawn::Item(name, data) => data.build(self, name),
        }
    }

    pub fn spawn_at(
        &mut self,
        spawn: &content::Spawn,
        place: impl Into<Place>,
    ) -> Entity {
        let e = self.spawn(spawn);
        e.place(self, place);
        e
    }

    pub fn spawn_raw(&mut self, loadout: impl hecs::DynamicBundle) -> Entity {
        Entity(self.ecs.spawn(loadout))
    }

    /// Spawns a new player entity if there isn't currently a player.
    pub fn spawn_player_at(&mut self, loc: Location) {
        if self.player.is_some() {
            return;
        }

        let player = Entity(self.ecs.spawn((
            Name("Fighter".into()),
            Icon('1'),
            Speed(4),
            Level(5),
            IsMob(true),
            IsFriendly(true),
            Stats {
                hit: 6,
                ev: 4,
                dmg: 4,
            },
        )));

        self.player = Some(player);
        player.place(self, loc);

        let party_spawns: Vec<(_, _)> = ["Ranger", "Monk", "Wizard"]
            .iter()
            .zip(self.perturbed_fill_positions(loc).skip(1))
            .collect();

        for (i, (name, loc)) in party_spawns.into_iter().enumerate() {
            let npc = Entity(self.ecs.spawn((
                Name((*name).into()),
                Icon(format!("{}", i + 2).chars().next().unwrap()),
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
            npc.place(self, loc);
            if *name == "Ranger" {
                self.wish(npc, "dagger").unwrap();
                self.wish(npc, "sword").unwrap();
                self.wish(npc, "magic map").unwrap();
            }
            if *name == "Monk" {
                for _ in 0..5 {
                    self.wish(npc, "potion of healing").unwrap();
                }
            }
            if *name == "Wizard" {
                for _ in 0..5 {
                    self.wish(npc, "scroll of fireball").unwrap();
                }
                for _ in 0..5 {
                    self.wish(npc, "scroll of lightning").unwrap();
                }
                for _ in 0..5 {
                    self.wish(npc, "scroll of confusion").unwrap();
                }
            }
            npc.set_goal(self, Goal::FollowPlayer);
        }
    }

    pub fn player(&self) -> Option<Entity> {
        self.player
    }

    pub fn live_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.placement.all_entities()
    }

    fn refresh_world_cache(&mut self, loc: Location) {
        for (loc, spawn) in self.world.populate_around(loc.into()) {
            self.spawn_at(&spawn, loc);
        }
    }

    /// Update the crate state by one tick.
    pub fn tick(&mut self) {
        // Start every tick by refreshing the world cache around the player's
        // position. If the player has moved to a location where new terrain
        // needs to be generated, that gets generated here.
        if let Some(loc) = self.player().and_then(|p| p.loc(self)) {
            self.refresh_world_cache(loc);
        }

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

            // Metabolize expired buffs.
            for buff in e.expired_buffs(self) {
                buff.expire_msg(self, e);
                e.with_mut::<Buffs, _>(self, |b| b.remove(&buff));
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
                    e.execute_indirect(self, act);
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
            loc.expanded_sector_locs().filter(|loc2| {
                !loc2.is_explored(self)
                    || (loc2.is_explored(self)
                        && loc2.is_walkable(self)
                        && loc2.sector() == loc.sector()
                        && s8::ns(*loc2).any(|loc| !loc.is_explored(self)))
            }),
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
                loc.flat_neighbors_4()
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
                loc.flat_neighbors_4()
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
                loc.perturbed_flat_neighbors_4()
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
        let dest = dest.follow(self);

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
        let dest = dest.follow(self);

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

    pub fn wish(
        &mut self,
        place: impl Into<Place>,
        name: &str,
    ) -> Option<Entity> {
        Some(self.spawn_at(&name.parse().ok()?, place))
    }
}
