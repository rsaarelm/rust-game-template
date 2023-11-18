use rand::prelude::*;
use serde::{Deserialize, Serialize};
use util::Logos;

use crate::{mapgen::Level, prelude::*, FlatPatch, Spawn};

/// Fixed-format data that specifies the contents of the initial game world.
/// Created from `WorldSpec`.
#[derive(Clone, Default)]
pub struct World {
    /// PRNG seed used
    seed: Logos,
    /// Map generation artifacts specifying terrain and entity spawns.
    patches: IndexMap<Location, FlatPatch>,
    /// Replicates data from `patches` in a more efficiently accessible form.
    terrain: HashMap<Location, Tile>,
}

impl TryFrom<WorldSpec> for World {
    type Error = anyhow::Error;

    fn try_from(value: WorldSpec) -> Result<Self, Self::Error> {
        let mut rng = util::srng(&value.seed);

        let mut patches = IndexMap::default();

        const MAX_DEPTH: u32 = 8;

        let mut prev_downstairs = None;
        for depth in 1..=MAX_DEPTH {
            let mut level = Level::new(depth);
            if depth < MAX_DEPTH {
                level = level.with_downstairs();
            }
            if let Some(p) = prev_downstairs {
                level = level.upstairs_at(p + ivec2(0, -1));
            }

            let map = level.sample(&mut rng);
            prev_downstairs = map.downstairs_pos();

            let z = -(depth as i16);
            patches.insert(Location::new(0, 0, z), map);
        }

        let mut terrain = HashMap::default();
        for (&loc, a) in &patches {
            for (pos, t) in a.tiles() {
                terrain.insert(loc + pos, t);
            }
        }

        Ok(World {
            seed: value.seed,
            patches,
            terrain,
        })
    }
}

impl World {
    pub fn spawns(&self) -> impl Iterator<Item = (Location, &'_ Spawn)> + '_ {
        self.patches.iter().flat_map(|(&loc, a)| {
            a.spawns.iter().map(move |(&p, s)| (loc + p, s))
        })
    }

    pub fn tile(&self, loc: &Location) -> Option<Tile> {
        self.terrain.get(loc).copied()
    }

    pub fn seed(&self) -> &Logos {
        &self.seed
    }

    pub fn entrance(&self) -> Option<Location> {
        for (&loc, a) in &self.patches {
            if let Some(pos) = a.entrance {
                return Some(loc + pos);
            }
        }
        None
    }
}

/// Compact description of what the initial game world is like. Will be stored
/// in save files. Expansion is highly context-dependent, may use prefab maps
/// or procedural generation.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct WorldSpec {
    seed: Logos,
}

impl WorldSpec {
    pub fn new(seed: Logos) -> Self {
        WorldSpec { seed }
    }
}
