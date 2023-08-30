use rand::prelude::*;
use serde::{Deserialize, Serialize};
use util::Logos;

use crate::{mapgen::Level, prelude::*, Patch, Spawn};

/// Data that specifies the contents of an initial game world.
#[derive(Clone, Default)]
pub struct World {
    seed: Logos,
    patches: IndexMap<Location, Patch>,
    terrain: HashMap<Location, Tile>,
}

impl TryFrom<WorldSpec> for World {
    type Error = anyhow::Error;

    fn try_from(value: WorldSpec) -> Result<Self, Self::Error> {
        let mut rng = util::srng(&value.seed);

        let mut patches = IndexMap::default();
        // TODO: Generate multiple floors when up and downstairs gen is
        // working.
        patches.insert(Location::new(0, 0, -1), Level::new(1).sample(&mut rng));

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

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct WorldSpec {
    seed: Logos,
}

impl WorldSpec {
    pub fn new(seed: Logos) -> Self {
        WorldSpec { seed }
    }
}
