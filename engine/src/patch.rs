use std::fmt;

use anyhow::bail;
use serde::{Deserialize, Serialize};
use util::Res;

use crate::{data::Germ, prelude::*};

/// Specification for a 2D patch of the game world.
#[derive(Clone, Default)]
pub struct Patch {
    pub terrain: HashMap<IVec2, Tile>,
    pub spawns: HashMap<IVec2, Res<&'static (dyn Germ + Sync + 'static)>>,
    pub entrance: Option<IVec2>,
}

impl fmt::Debug for Patch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Patch TODO print")
    }
}

impl Patch {
    pub fn merge(&mut self, offset: IVec2, other: Patch) {
        for (p, t) in other.terrain {
            self.terrain.insert(p + offset, t);
        }

        for (p, s) in other.spawns {
            self.spawns.insert(p + offset, s);
        }

        if self.entrance.is_none() {
            self.entrance = other.entrance.map(|p| p + offset);
        }
    }
}

/// Datafile version of `Patch`.
#[derive(Clone, Default, Debug, Deserialize)]
pub struct PatchData {
    pub map: String,
    pub legend: IndexMap<char, Res<&'static (dyn Germ + Sync + 'static)>>,
}

impl TryFrom<PatchData> for Patch {
    type Error = anyhow::Error;

    fn try_from(value: PatchData) -> Result<Self, Self::Error> {
        eprintln!("Blag!");
        let mut terrain = HashMap::default();
        let mut spawns = HashMap::default();
        let mut entrance = None;
        for (y, line) in value.map.lines().enumerate() {
            for (x, c) in line.chars().enumerate() {
                if c.is_whitespace() {
                    continue;
                }

                let p = ivec2(x as i32, y as i32);
                if c == '@' {
                    entrance = Some(p);
                } else if let Some(s) = value.legend.get(&c) {
                    // XXX: Can't do this here since patches are loaded during
                    // initial data deserialization and other data must not be
                    // touched through res handles yet... Have to punt it to
                    // applying the patch.
                    //terrain.insert(p, s.preferred_tile());
                    spawns.insert(p, s.clone());
                } else if let Ok(t) = Tile::try_from(c) {
                    terrain.insert(p, t);
                } else {
                    bail!("Bad patch char {c:?}");
                }
            }
        }

        Ok(Patch {
            terrain,
            spawns,
            entrance,
        })
    }
}

impl<'de> Deserialize<'de> for Patch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = PatchData::deserialize(deserializer)?;
        Patch::try_from(data).map_err(serde::de::Error::custom)
    }
}
