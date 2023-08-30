use std::collections::BTreeMap;

use anyhow::bail;
use derive_deref::Deref;
use serde::{Deserialize, Serialize, Serializer};
use util::{_String, s8};

use crate::{data::StaticGerm, prelude::*, Rect};

/// Specification for a 2D patch of the game world.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Patch {
    pub terrain: IndexMap<IVec2, Tile>,
    pub spawns: IndexMap<IVec2, Spawn>,
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

    pub fn set_terrain(&mut self, pos: impl Into<IVec2>, t: Tile) {
        self.terrain.insert(pos.into(), t);
    }

    pub fn add_spawn(&mut self, pos: impl Into<IVec2>, spawn: Spawn) {
        self.spawns.insert(pos.into(), spawn);
    }

    pub fn bounds(&self) -> Rect {
        Rect::from_points_inclusive(
            self.terrain
                .iter()
                .filter_map(|(&p, &t)| (t != Tile::Wall).then_some(p)),
        )
    }

    fn is_solid(&self, pos: IVec2) -> bool {
        self.terrain.get(&pos).map_or(true, |&t| t == Tile::Wall)
    }

    fn has_tunnel_support(&self, pos: IVec2) -> bool {
        //  #..
        //  #@.
        //  ###
        //
        // The forbidden tunneling position, digging at @ will open an
        // un-tunnely 4-cell square.

        for d in [0, 2, 4, 6] {
            if (d..d + 3).all(|a| !self.is_solid(pos + s8::DIR[a % 8])) {
                return false;
            }
        }
        true
    }

    pub fn is_tunnel(&self, pos: IVec2) -> bool {
        self.has_tunnel_support(pos)
            && self.spawns.get(&pos).is_none()
            && self.terrain.get(&pos) == Some(&Tile::Ground)
    }

    pub fn can_tunnel(&self, pos: IVec2) -> bool {
        match self.terrain.get(&pos) {
            // Already open, pass through
            Some(t) if t.is_walkable() => true,
            None if self.has_tunnel_support(pos) => true,
            _ => false,
        }
    }

    pub fn open_area(&self) -> impl Iterator<Item = IVec2> + '_ {
        self.terrain
            .iter()
            .filter_map(|(p, t)| t.is_walkable().then_some(*p))
    }

    pub fn downstairs_pos(&self) -> Option<IVec2> {
        self.terrain
            .iter()
            .find(|(_, &t)| t == Tile::Downstairs)
            .map(|(&p, _)| p)
    }

    // See if the patch can be placed in given offset without clobbering
    // existing area.
    pub fn can_place(&self, offset: IVec2, other: &Patch) -> bool {
        // TODO: avoid placements where more than one consecutive chunk is
        // taken from open edge.
        for (&p, &t) in &other.terrain {
            // Cell isn't defined locally yet, good to go.
            let Some(&current) = self.terrain.get(&(p + offset)) else {
                continue;
            };

            // Both cells are defined, but both are wall. Walls can merge.
            if current == Tile::Wall && t == Tile::Wall {
                continue;
            }

            // You can drop walkable tiles on top of a tunnel.
            if self.is_tunnel(p + offset) && t.is_walkable() {
                continue;
            }

            // Otherwise it's a clash, bail out.
            return false;
        }

        true
    }

    pub fn tiles(&self) -> impl Iterator<Item = (IVec2, Tile)> + '_ {
        let overlay: HashMap<IVec2, Tile> = self
            .spawns
            .iter()
            .map(|(&p, s)| (p, s.preferred_tile()))
            .collect();

        self.terrain.iter().map(move |(&p, &t)| {
            if let Some(&t) = overlay.get(&p) {
                (p, t)
            } else {
                (p, t)
            }
        })
    }
}

/// Datafile version of `Patch`.
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct PatchData {
    pub legend: BTreeMap<char, Spawn>,
}

impl TryFrom<((PatchData,), String)> for Patch {
    type Error = anyhow::Error;

    fn try_from(
        ((data,), map): ((PatchData,), String),
    ) -> Result<Self, Self::Error> {
        let mut terrain = IndexMap::default();
        let mut spawns = IndexMap::default();
        let mut entrance = None;
        for (y, line) in map.lines().enumerate() {
            for (x, c) in line.chars().enumerate() {
                if c.is_whitespace() {
                    continue;
                }

                let p = ivec2(x as i32, y as i32);
                if c == '@' {
                    entrance = Some(p);
                    // Assume that player always stands on regular ground.
                    terrain.insert(p, Tile::Ground);
                } else if let Some(s) = data.legend.get(&c) {
                    spawns.insert(p, s.clone());
                    // NB. Spawn data can't be safely accessed at the point
                    // where patches are being instantiated, because both are
                    // found in the static gamedata. At this point assume that
                    // all spawns will have regular ground under them, and
                    // rewrite the terrain in patch applying stage if the
                    // concrete spawn turns out to want something weird
                    // instead.
                    terrain.insert(p, Tile::Ground);
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

impl From<&Patch> for ((PatchData,), String) {
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
                } else if let Some(&t) = value.terrain.get(&p) {
                    map.push(t.into());
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

        ((PatchData { legend },), map)
    }
}

impl<'de> Deserialize<'de> for Patch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = <((PatchData,), String)>::deserialize(deserializer)?;
        Patch::try_from(data).map_err(serde::de::Error::custom)
    }
}

impl Serialize for Patch {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        <((PatchData,), String)>::from(self).serialize(serializer)
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
    pub fn preferred_tile(&self) -> Tile {
        self.0.parse::<StaticGerm>().unwrap().preferred_tile()
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

impl From<&_String> for Spawn {
    fn from(value: &_String) -> Self {
        Spawn((**value).clone())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn patch_roundtrip() {
        const PATCH: &str = "\
:legend
  a archon
  x xorn
 ###
##a##
#..x#
##@##
 ###";

        let p: Patch = idm::from_str(PATCH).unwrap();
        let reser = idm::to_string(&p).unwrap();
        assert_eq!(PATCH, reser.trim_end());
    }

    #[test]
    fn legend_assign() {
        const PATCH: &str = "\
:legend
  x alien-one
  y alien-two
  z alien-three
 ###
##x##
#yz.#
##@##
 ###";

        let p: Patch = idm::from_str(PATCH).unwrap();
        let reser = idm::to_string(&p).unwrap();
        let p2: Patch = idm::from_str(&reser).unwrap();
        assert_eq!(p, p2);
    }
}
