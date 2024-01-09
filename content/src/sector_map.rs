use anyhow::bail;
use glam::{ivec3, IVec2};
use serde::{Deserialize, Serialize};
use util::{HashMap, IndexMap, _String, text, Cloud};

use crate::{Block, Coordinates, Cube, Environs, Location, Spawn, Voxel};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SectorMap {
    name: String,
    map: String,
    // Values are spawns, but store them as strings here since they can't be
    // validated until gamedata has been completely loaded.
    legend: IndexMap<char, String>,
}

impl SectorMap {
    pub fn from_area<'a, 'b>(
        r: &'a impl Environs,
        volume: &Cube,
        spawns: impl IntoIterator<Item = (&'b Location, &'b Spawn)>,
    ) -> Self {
        // XXX This is kinda hacky, mostly intended for visualization of
        // generated maps, not actual gameplay use.
        let mut map = String::new();
        let z = volume.min()[2];

        const LEGEND_ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                                       abcdefghijklmnopqrstuvwxyz\
                                       αβγδεζηθικλμξπρστφχψω\
                                       ΓΔΛΞΠΣΦΨΩ\
                                       БГҐДЂЃЄЖЗЙЛЉЊПЎФЦЧЏШЩЪЭЮЯ\
                                       àèòùáêõýþâìúãíäîåæçéóëïðñôûöøüÿ\
                                       ÀÈÒÙÁÊÕÝÞÂÌÚÃÉÓÄÍÅÆÇËÎÔÏÐÑÖØÛßÜ";

        let mut extra_letters: Vec<char> =
            LEGEND_ALPHABET.chars().rev().collect();

        let mut rev_legend: HashMap<String, char> = Default::default();

        let mut map_spawns = HashMap::default();
        for (loc, spawn) in spawns {
            let loc = ivec3(loc.x, loc.y, z);
            let name = _String::from(spawn.clone()).0;

            let c = if let Some(c) = rev_legend.get(&name) {
                *c
            } else {
                let c = extra_letters.pop().expect("Out of letters for legend");
                rev_legend.insert(name, c);
                c
            };

            map_spawns.insert(loc, c);
        }

        for y in volume.min()[1]..volume.max()[1] {
            for x in volume.min()[0]..volume.max()[0] {
                use Block::*;

                let p = ivec3(x, y, z);

                if let Some(c) = map_spawns.get(&p) {
                    map.push(*c);
                    continue;
                }

                let c = match p.tile(r) {
                    crate::Tile::Surface(loc, _) if loc == p.above() => '<',
                    crate::Tile::Surface(loc, _) if loc == p.below() => '>',
                    crate::Tile::Surface(_, Water) => '~',
                    crate::Tile::Surface(_, Magma) => '&',
                    crate::Tile::Surface(_, Grass) => ',',
                    crate::Tile::Surface(_, SplatteredRock) => '§',
                    crate::Tile::Surface(_, _) => '.',
                    crate::Tile::Wall(Door) => '+',
                    crate::Tile::Wall(Glass) => '|',
                    crate::Tile::Wall(_) => '#',
                    crate::Tile::Void => '_',
                };
                map.push(c);
            }
            map.push('\n');
        }
        // TODO Generate legend.

        Self {
            name: Default::default(),
            map,
            legend: rev_legend.into_iter().map(|(k, v)| (v, k)).collect(),
        }
    }

    pub fn entrances(&self) -> impl Iterator<Item = IVec2> + '_ {
        text::char_grid(&self.map).filter_map(|(p, c)| (c == '@').then_some(p))
    }

    pub fn downstairs(&self) -> Option<IVec2> {
        text::char_grid(&self.map).find_map(|(p, c)| (c == '>').then_some(p))
    }

    pub fn spawns(
        &self,
        origin: &Location,
    ) -> anyhow::Result<Vec<(Location, Spawn)>> {
        let mut ret = Vec::default();

        for (p, c) in text::char_grid(&self.map) {
            if let Some(name) = self.legend.get(&c) {
                ret.push((*origin + p.extend(0), name.parse()?));
            }
        }

        Ok(ret)
    }

    pub fn terrain(
        &self,
        origin: &Location,
    ) -> anyhow::Result<Cloud<3, Voxel>> {
        use Block::*;

        let mut ret = Cloud::default();

        for (p, c) in text::char_grid(&self.map) {
            let p = *origin + p.extend(0);

            let c = match c {
                // Rewrite entrace cells.
                '@' => '.',
                // Assume all spawns spawn on top of regular ground
                c if self.legend.contains_key(&c) => '.',
                c => c,
            };

            match c {
                '#' => {
                    ret.insert(p.above(), Some(Rock));
                    ret.insert(p, Some(Rock));
                    ret.insert(p.below(), Some(Rock));
                }
                '+' => {
                    ret.insert(p.above(), Some(Rock));
                    ret.insert(p, Some(Door));
                    ret.insert(p.below(), Some(Rock));
                }
                '|' => {
                    ret.insert(p.above(), Some(Rock));
                    ret.insert(p, Some(Glass));
                    ret.insert(p.below(), Some(Rock));
                }
                '.' => {
                    ret.insert(p, None);
                    ret.insert(p.below(), Some(Rock));
                }
                '~' => {
                    ret.insert(p, None);
                    ret.insert(p.below(), Some(Water));
                }
                '&' => {
                    ret.insert(p, None);
                    ret.insert(p.below(), Some(Magma));
                }
                '>' | '_' => {
                    ret.insert(p, None);
                    ret.insert(p.below(), None);
                }
                '<' => {
                    ret.insert(p.above(), None);
                    ret.insert(p, Some(Rock));
                    ret.insert(p.below(), Some(Rock));
                }
                _ => bail!("Unknown terrain {c:?}"),
            };
        }

        Ok(ret)
    }
}
