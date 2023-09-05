//! Static game data

use std::{str::FromStr, sync::LazyLock};

use anyhow::bail;
use serde::Deserialize;
use util::{IndexMap, _String};

use crate::{ecs::*, item::ItemKind, prelude::*, Patch};

/// Static global game data.
#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Data {
    pub bestiary: IndexMap<_String, Monster>,
    pub armory: IndexMap<_String, Item>,
    pub vaults: IndexMap<_String, Patch>,
}

// Custom loader that initializes the global static gamedata from the data
// files. The data.idm.z file is constructed from project data files by engine
// crate's build.rs script.
impl Default for &'static Data {
    fn default() -> Self {
        static DATA: LazyLock<Data> = LazyLock::new(|| {
            let data = fdeflate::decompress_to_vec(include_bytes!(
                "../../target/data.idm.z"
            ))
            .unwrap();
            let data = std::str::from_utf8(&data).unwrap();
            let data: Data = idm::from_str(data).unwrap();

            data
        });

        &DATA
    }
}

impl Data {
    pub fn get() -> &'static Data {
        Default::default()
    }
}

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Monster {
    pub icon: char,
    pub might: i32,
    pub rarity: u32,
    pub min_depth: u32,
}

impl Germ for Monster {
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

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Item {
    pub might: i32,
    pub kind: ItemKind,
    pub rarity: u32,

    #[serde(with = "util::dash_option")]
    pub power: Option<Power>,
}

impl Germ for Item {
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
pub trait Germ {
    fn build(&self, r: &mut Runtime) -> Entity;

    /// What kind of terrain does this thing like to spawn on.
    ///
    /// Usually things spawn on ground, but eg. aquatic monsters might be
    /// spawning on water instead. Having this lets us do maps where the
    /// terrain cell is not specified for germ locations.
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

pub type StaticGerm = &'static (dyn Germ + Sync + 'static);

impl FromStr for StaticGerm {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Magic switchboard that trawls the data files looking for named
        // things that can be spawned.
        if let Some(monster) = Data::get().bestiary.get(s) {
            return Ok(monster as StaticGerm);
        }

        if let Some(item) = Data::get().armory.get(s) {
            return Ok(item as StaticGerm);
        }

        bail!("Unknown germ {s:?}")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn load_data() {
        // This test will crash if the static gamedata won't deserialize
        // cleanly.
        assert!(!Data::get().bestiary.is_empty());
    }
}
