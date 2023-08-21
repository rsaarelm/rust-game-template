use std::collections::BTreeMap;

use anyhow::bail;
use derive_deref::Deref;
use serde::{Deserialize, Serialize, Serializer};

use crate::{data::StaticGerm, prelude::*, Rect};

/// Specification for a 2D patch of the game world.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Patch {
    pub terrain: HashMap<IVec2, Tile>,
    pub spawns: HashMap<IVec2, Spawn>,
    pub entrance: Option<IVec2>,
}

impl Patch {
    /// Merge another patch into this.
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

    /// Apply patch to world.
    ///
    /// If world does not have player defined and the patch specifies an entry
    /// point, a player will be spawned at that point.
    pub fn apply(&self, r: &mut Runtime, origin: Location) {
        for (&p, &t) in &self.terrain {
            let loc = origin + p;
            loc.set_tile(r, t);
        }

        // Set all the spawn tiles before spawning things so spawns will have
        // maximally complete patch to show up in.
        for (&p, s) in &self.spawns {
            s.prepare_terrain(r, origin + p);
        }

        for (&p, s) in &self.spawns {
            s.spawn(r, origin + p);
        }

        if let Some(p) = self.entrance {
            // spawn_player is assumed to be a no-op if player already exists.
            r.spawn_player(origin + p);
        }
    }
}

/// Datafile version of `Patch`.
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct PatchData {
    pub map: String,
    pub legend: BTreeMap<char, Spawn>,
}

impl TryFrom<PatchData> for Patch {
    type Error = anyhow::Error;

    fn try_from(value: PatchData) -> Result<Self, Self::Error> {
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
                    // Assume that player always stands on regular ground.
                    terrain.insert(p, Tile::Ground);
                } else if let Some(s) = value.legend.get(&c) {
                    // XXX: It would be nice to put the preferred terrain for
                    // the spawn thing down at this point, but vault patches
                    // are loaded during initial static gamedata
                    // initialization when the data isn't available yet.
                    // Terrain setting needs to be punted into the point where
                    // the patch is applied to a runtime then.
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

impl From<&Patch> for PatchData {
    fn from(value: &Patch) -> Self {
        const LEGEND_ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                                       abcdefghijklmnopqrstuvwxyz\
                                       αβγδεζηθικλμξπρστφχψω\
                                       ΓΔΛΞΠΣΦΨΩ\
                                       БГҐДЂЃЄЖЗЙЛЉЊПЎФЦЧЏШЩЪЭЮЯ\
                                       àèòùáêõýþâìúãíäîåæçéóëïðñôûöøüÿ\
                                       ÀÈÒÙÁÊÕÝÞÂÌÚÃÉÓÄÍÅÆÇËÎÔÏÐÑÖØÛßÜ";

        let mut extra_letters: Vec<char> =
            LEGEND_ALPHABET.chars().rev().collect();

        let mut legend: HashMap<String, char> = Default::default();

        let bounds = Rect::from_points_inclusive(
            value.terrain.keys().copied().chain(
                value
                    .spawns
                    .keys()
                    .copied()
                    .chain(value.entrance.iter().copied()),
            ),
        );

        let mut map = String::new();
        for y in bounds.min()[1]..bounds.max()[1] {
            let mut seen_content = false;
            for x in bounds.min()[0]..bounds.max()[0] {
                let p = ivec2(x, y);
                if value.entrance == Some(p) {
                    map.push('@');
                    seen_content = true;
                } else if let Some(&t) = value.terrain.get(&p) {
                    map.push(t.into());
                    seen_content = true;
                } else if let Some(s) = value.spawns.get(&p) {
                    if let Some(&c) = legend.get(&s.0) {
                        // Already established a legend char, reuse.
                        map.push(c);
                    } else {
                        // Assign a new legend char.
                        let mut c = s
                            .chars()
                            .find(|a| a.is_alphabetic())
                            .unwrap_or('A');
                        if let Some(p) =
                            extra_letters.iter().position(|&a| a == c)
                        {
                            // We can use the initial.
                            extra_letters.swap_remove(p);
                        } else {
                            // It's already in use, let's use a random letter.
                            c = extra_letters.pop().expect(
                                "patch generator ran out of legend chars",
                            );
                        }
                        legend.insert(s.to_string(), c);

                        map.push(c);
                    }
                    seen_content = true;
                } else if !seen_content {
                    // Push NBSP to make initial space not look like
                    // indentation to IDM.
                    map.push(' ');
                } else {
                    // No longer at start, can use regular space now.
                    map.push(' ');
                }
            }
            // Remove trailing space
            let len = map.trim_end().len();
            map.truncate(len);

            if y != bounds.max()[1] - 1 {
                map.push('\n');
            }
        }

        // Reverse legend
        let legend = legend.into_iter().map(|(n, c)| (c, Spawn(n))).collect();

        PatchData { map, legend }
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

impl Serialize for Patch {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        PatchData::from(self).serialize(serializer)
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Deref,
    Serialize,
    Deserialize,
)]
pub struct Spawn(String);

impl Spawn {
    pub fn prepare_terrain(&self, r: &mut Runtime, loc: Location) {
        let germ: StaticGerm = self.0.parse().unwrap();

        if loc.tile(r) == Tile::default() {
            loc.set_tile(r, germ.preferred_tile());
        }
    }

    pub fn spawn(&self, r: &mut Runtime, loc: Location) -> Entity {
        let germ: StaticGerm = self.0.parse().unwrap();
        let e = germ.build(r);

        // Names are map keys so they're not stored in the germ, assign the
        // name here.
        e.set(r, crate::ecs::Name(self.0.clone()));
        e.place(r, loc);

        e
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn patch_roundtrip() {
        const PATCH: &str = "\
map
   ###
  ##a##
  #..x#
  ##@##
   ###
legend
  a archon
  x xorn";

        let p: Patch = idm::from_str(PATCH).unwrap();
        let reser = idm::to_string(&p).unwrap();
        assert_eq!(PATCH, reser.trim_end());
    }

    #[test]
    fn legend_assign() {
        const PATCH: &str = "\
map
   ###
  ##x##
  #yz.#
  ##@##
   ###
legend
  x alien-one
  y alien-two
  z alien-three";

        let p: Patch = idm::from_str(PATCH).unwrap();
        let reser = idm::to_string(&p).unwrap();
        let p2: Patch = idm::from_str(&reser).unwrap();
        assert_eq!(p, p2);
    }
}
