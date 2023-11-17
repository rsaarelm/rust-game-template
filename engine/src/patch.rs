use std::collections::BTreeMap;

use anyhow::bail;
use derive_more::Deref;
use serde::{Deserialize, Serialize, Serializer};
use util::{_String, s4, s8};

use crate::{data::StaticSeed, placement::Place, prelude::*, Rect};

// TODO Replace with a 3D patch struct in the future
pub type Patch = (Location, FlatPatch);

/// Specification for a 2D patch of the game world.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FlatPatch {
    pub terrain: IndexMap<IVec2, MapTile>,
    pub spawns: IndexMap<IVec2, Spawn>,
    pub entrance: Option<IVec2>,
}

impl FlatPatch {
    /// Merge another patch into this.
    pub fn merge(&mut self, offset: IVec2, other: FlatPatch) {
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

    pub fn set_terrain(&mut self, pos: impl Into<IVec2>, t: MapTile) {
        self.terrain.insert(pos.into(), t);
    }

    pub fn add_spawn(&mut self, pos: impl Into<IVec2>, spawn: Spawn) {
        self.spawns.insert(pos.into(), spawn);
    }

    pub fn bounds(&self) -> Rect {
        Rect::from_points_inclusive(
            self.terrain
                .iter()
                .filter_map(|(&p, &t)| (t != MapTile::Wall).then_some(p)),
        )
    }

    fn is_solid(&self, pos: IVec2) -> bool {
        self.terrain.get(&pos).map_or(true, |&t| t == MapTile::Wall)
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
            && self.terrain.get(&pos) == Some(&MapTile::Ground)
    }

    pub fn valid_tunnels_from(
        &self,
        &pos: &IVec2,
    ) -> impl Iterator<Item = IVec2> + '_ {
        let open = |p| self.terrain.get(&p).map_or(false, |t| t.is_walkable());

        s4::ns(pos).filter(move |&p2| {
            // Already open neighbor, pass right through.
            if open(p2) {
                return true;
            }

            let front = p2 - pos;
            let side = front.rotate(ivec2(0, 1));

            // Two consecutive open cells on either side, no go.
            if open(pos + side) && open(pos + side + front) {
                return false;
            }

            if open(pos - side) && open(pos - side + front) {
                return false;
            }

            // Don't dig through defined walls.
            if self.terrain.get(&p2).is_some() {
                return false;
            }

            true
        })
    }

    pub fn open_area(&self) -> impl Iterator<Item = IVec2> + '_ {
        self.terrain
            .iter()
            .filter_map(|(p, t)| t.is_walkable().then_some(*p))
    }

    pub fn is_open(&self, pos: IVec2) -> bool {
        self.terrain.get(&pos).map_or(false, |t| t.is_walkable())
    }

    pub fn downstairs_pos(&self) -> Option<IVec2> {
        self.terrain
            .iter()
            .find(|(_, &t)| t == MapTile::Downstairs)
            .map(|(&p, _)| p)
    }

    // See if the patch can be placed in given offset without clobbering
    // existing area.
    pub fn can_place(&self, offset: IVec2, other: &FlatPatch) -> bool {
        // TODO: avoid placements where more than one consecutive chunk is
        // taken from open edge.
        for (&p, &t) in &other.terrain {
            // Cell isn't defined locally yet, good to go.
            let Some(&current) = self.terrain.get(&(p + offset)) else {
                continue;
            };

            // Both cells are defined, but both are wall. Walls can merge.
            if current == MapTile::Wall && t == MapTile::Wall {
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

    pub fn tiles(&self) -> impl Iterator<Item = (IVec2, MapTile)> + '_ {
        let overlay: HashMap<IVec2, MapTile> = self
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

    pub fn upstair_positions(&self) -> impl Iterator<Item = IVec2> + '_ {
        self.open_area().map(|p| p + ivec2(0, -1)).filter(|&p| {
            self.terrain.get(&p).is_none()
                && !self.is_open(p + ivec2(-1, 0))
                && !self.is_open(p + ivec2(1, 0))
                && !self.is_open(p + ivec2(-1, -1))
                && !self.is_open(p + ivec2(0, -1))
                && !self.is_open(p + ivec2(1, -1))
        })
    }

    pub fn downstair_positions(&self) -> impl Iterator<Item = IVec2> + '_ {
        self.open_area().map(|p| p + ivec2(0, 1)).filter(|&p| {
            self.terrain.get(&p).is_none()
                && !self.is_open(p + ivec2(-1, 0))
                && !self.is_open(p + ivec2(1, 0))
                && !self.is_open(p + ivec2(-1, 1))
                && !self.is_open(p + ivec2(0, 1))
                && !self.is_open(p + ivec2(1, 1))
        })
    }
}

/// Datafile version of `Patch`.
#[derive(Clone, Default, Debug, Deserialize, Serialize)]
pub struct PatchData {
    pub legend: BTreeMap<char, Spawn>,
}

impl TryFrom<((PatchData,), String)> for FlatPatch {
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
                    terrain.insert(p, MapTile::Ground);
                } else if let Some(s) = data.legend.get(&c) {
                    spawns.insert(p, s.clone());
                    // NB. Spawn data can't be safely accessed at the point
                    // where patches are being instantiated, because both are
                    // found in the static gamedata. At this point assume that
                    // all spawns will have regular ground under them, and
                    // rewrite the terrain in patch applying stage if the
                    // concrete spawn turns out to want something weird
                    // instead.
                    terrain.insert(p, MapTile::Ground);
                } else if let Ok(t) = MapTile::try_from(c) {
                    terrain.insert(p, t);
                } else {
                    bail!("Bad patch char {c:?}");
                }
            }
        }

        Ok(FlatPatch {
            terrain,
            spawns,
            entrance,
        })
    }
}

impl From<&FlatPatch> for ((PatchData,), String) {
    fn from(value: &FlatPatch) -> Self {
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
                    map.push('\u{00A0}');
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

impl<'de> Deserialize<'de> for FlatPatch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = <((PatchData,), String)>::deserialize(deserializer)?;
        FlatPatch::try_from(data).map_err(serde::de::Error::custom)
    }
}

impl Serialize for FlatPatch {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        <((PatchData,), String)>::from(self).serialize(serializer)
    }
}

/// Representation of a generatable entity.
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
    pub fn preferred_tile(&self) -> MapTile {
        self.0.parse::<StaticSeed>().unwrap().preferred_tile()
    }

    pub fn spawn(
        &self,
        r: &mut impl AsMut<Runtime>,
        place: impl Into<Place>,
    ) -> Entity {
        let r = r.as_mut();
        r.wish(place, &self.0).unwrap()
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

        let p: FlatPatch = idm::from_str(PATCH).unwrap();
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

        let p: FlatPatch = idm::from_str(PATCH).unwrap();
        let reser = idm::to_string(&p).unwrap();
        let p2: FlatPatch = idm::from_str(&reser).unwrap();
        assert_eq!(p, p2);
    }
}
