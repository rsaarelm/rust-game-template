use anyhow::Result;
use content::{Data, Environs, Voxel, World};
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use util::{GameRng, Silo};

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
    pub fn new(seed: Silo) -> Result<Self> {
        let world = World::new(seed, Data::get().scenario.clone())?;
        let rng = util::srng(world.seed());

        let mut ret = Runtime {
            world,
            rng,
            ..Default::default()
        };

        let entrance = ret.world.player_entrance();
        // Construct the initial world space and create the spawns.
        ret.bump_cache_at(entrance);
        ret.spawn_player_at(entrance);

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
        e.place(self, e.open_placement_spot(self, place));
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
            Name("Player".into()),
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
        let sword = self.wish(player, "sword").unwrap();
        player.make_equipped(self, &sword);
    }

    pub fn player(&self) -> Option<Entity> {
        self.player
    }

    pub fn live_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.placement.all_entities()
    }

    /// Do a cache update around the player character's current location.
    pub fn bump_cache(&mut self) {
        if let Some(loc) = self.player().and_then(|p| p.loc(self)) {
            self.bump_cache_at(loc);
        }
    }

    fn bump_cache_at(&mut self, loc: Location) {
        for (loc, spawn) in self.world.populate_around(loc) {
            self.spawn_at(&spawn, loc);
        }
    }

    /// Update the crate state by one tick.
    pub fn tick(&mut self) {
        // Start every tick by refreshing the world cache around the player's
        // position. If the player has moved to a location where new terrain
        // needs to be generated, that gets generated here.
        self.bump_cache();

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
                        Goal::GoTo { .. } => e.next_goal(self),
                        Goal::Autoexplore(_) => e.next_goal(self),
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

impl Environs for Runtime {
    fn voxel(&self, loc: Location) -> Voxel {
        self.world.get(loc)
    }

    fn set_voxel(&mut self, loc: Location, voxel: Voxel) {
        self.world.set(loc, voxel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn build_world() {
        let runtime = Runtime::new(Silo::new("rand0m")).unwrap();
        assert!(runtime.player().is_some());
    }

    #[test]
    fn saving_and_loading() {
        let runtime = Runtime::new(Silo::new("rand0m")).unwrap();
        let save = idm::to_string(&runtime).expect("Save failed");
        let runtime2: Runtime = idm::from_str(&save).expect("Load failed");
        // Check that roundtrip keeps it same.
        assert_eq!(save, idm::to_string(&runtime2).unwrap());
    }
}
