//! Static game data

use std::str::FromStr;

use anyhow::bail;
use content::{Data, Item, Monster};

use crate::{ecs::*, prelude::*};

impl EntitySeed for Monster {
    fn build(&self, r: &mut Runtime) -> Entity {
        Entity(r.ecs.spawn((
            Icon(self.icon),
            Speed(3),
            Level(self.might),
            IsMob(true),
            Stats {
                hit: self.might,
                ev: self.might / 2,
                dmg: self.might,
            },
        )))
    }

    fn rarity(&self) -> u32 {
        self.rarity
    }

    fn min_depth(&self) -> u32 {
        self.min_depth
    }
}

impl EntitySeed for Item {
    fn build(&self, r: &mut Runtime) -> Entity {
        Entity(r.ecs.spawn((
            Icon(self.kind.icon()),
            ItemPower(self.power),
            self.kind,
            Level(self.might),
        )))
    }
}

/// Values that specify new entities to be created.
pub trait EntitySeed {
    fn build(&self, r: &mut Runtime) -> Entity;

    /// What kind of terrain does this thing like to spawn on.
    ///
    /// Usually things spawn on ground, but eg. aquatic monsters might be
    /// spawning on water instead. Having this lets us do maps where the
    /// terrain cell is not specified for seed locations.
    fn preferred_tile(&self) -> Tile {
        Tile::Ground
    }

    fn rarity(&self) -> u32 {
        1
    }

    fn spawn_weight(&self) -> f64 {
        match self.rarity() {
            0 => 0.0,
            r => 1.0 / r as f64,
        }
    }

    fn min_depth(&self) -> u32 {
        0
    }
}

pub type StaticSeed = &'static (dyn EntitySeed + Sync + 'static);

impl FromStr for StaticSeed {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Magic switchboard that trawls the data files looking for named
        // things that can be spawned.
        if let Some(monster) = Data::get().bestiary.get(s) {
            return Ok(monster as StaticSeed);
        }

        if let Some(item) = Data::get().armory.get(s) {
            return Ok(item as StaticSeed);
        }

        bail!("Unknown seed {s:?}")
    }
}
