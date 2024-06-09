use std::ops::Deref;

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
        let world = World::new(
            seed,
            Data::get().campaign.iter().next().unwrap().1.clone(),
        )?;
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

    /// Spawn a single pod object.
    ///
    /// Since objects specify counts but the entity type might not be
    /// stackable, this can still produce multiple entities, so it returns a
    /// vector.
    fn spawn_object(&mut self, object: &content::PodObject) -> Vec<Entity> {
        // Build the base entity before multiplication.
        let entity = match &object.kind {
            content::PodKind::Monster(data) => data.build(self, &object.name),
            content::PodKind::Item(data) => data.build(self, &object.name),
        };

        let mut ret = vec![entity];

        if object.count > 1 {
            if entity.can_stack_with(self, &entity) {
                // It's stackable, just set the multiple and we're done.
                entity.set(self, Count(object.count));
            } else {
                // Otherwise we make a bunch of cloned entities.
                for _ in 1..object.count {
                    ret.push(entity.spawn_clone(self));
                }
            }
        }

        ret
    }

    pub fn spawn_at(
        &mut self,
        pod: &content::Pod,
        place: impl Into<Place>,
    ) -> Vec<Entity> {
        let place = place.into();

        let mut ret = Vec::new();
        for ((o,), contents) in pod.deref() {
            let es = self.spawn_object(o);

            for e in es {
                // Recursively generate contents.
                let contents = self.spawn_at(contents, e);

                // Autoequip contents in order for mobs.
                if e.is_mob(self) {
                    for i in contents {
                        e.make_equipped(self, &i);
                    }
                }

                e.place(self, e.open_placement_spot(self, place));
                ret.push(e);
            }
        }
        ret
    }

    pub fn spawn_raw(&mut self, loadout: impl hecs::DynamicBundle) -> Entity {
        Entity(self.ecs.spawn(loadout))
    }

    /// Spawns a new player entity if there isn't currently a player.
    pub fn spawn_player_at(&mut self, loc: Location) {
        if self.player.is_some() {
            return;
        }

        let players = self.spawn_at(&Data::get().loadout, loc);

        if players.is_empty() {
            panic!("Loadout does not define any characters");
        }

        for p in players {
            p.set(self, IsFriendly(true));

            // Set the first creature as the current player.
            if self.player.is_none() {
                self.player = Some(p);
                // XXX: We need to do this again to register the player's
                // initial FOV.
                p.post_move_hook(self);
            }
        }
    }

    pub fn player(&self) -> Option<Entity> {
        self.player
    }

    pub fn live_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.placement.all_entities()
    }

    pub fn entities(
        &self,
        place: impl Into<Place>,
    ) -> Box<dyn Iterator<Item = Entity> + '_> {
        match place.into() {
            Place::In(e) => Box::new(self.placement.entities_in(&e)),
            Place::At(loc) => Box::new(self.placement.entities_at(loc)),
        }
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
        // TODO Handle wishes that produce multiple entities
        Some(self.spawn_at(&name.parse().ok()?, place)[0])
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
        content::register_data_from("../data").unwrap();

        let runtime = Runtime::new(Silo::new("rand0m")).unwrap();
        assert!(runtime.player().is_some());
    }

    #[test]
    fn saving_and_loading() {
        content::register_data_from("../data").unwrap();

        let runtime = Runtime::new(Silo::new("rand0m")).unwrap();
        let save = idm::to_string(&runtime).expect("Save failed");
        let runtime2: Runtime = idm::from_str(&save).expect("Load failed");
        // Check that roundtrip keeps it same.
        assert_eq!(save, idm::to_string(&runtime2).unwrap());
    }
}
