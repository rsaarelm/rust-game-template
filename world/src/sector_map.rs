use std::ops::Deref;

use glam::{IVec2, ivec2, ivec3};
use rand::distr::{Distribution, StandardUniform};
use serde::{Deserialize, Serialize};
use util::{Cloud, HashMap, IndexMap, LazyRes, Neighbors2D, StrExt};

use crate::{Block, Coordinates, Cube, Environs, Location, Pod, Voxel};

/// Text map for 2D world part.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SectorMap {
    pub name: String,
    pub map: String,
    // Use LazyRes for values since clutches can't be parsed until gamedata has
    // been loaded.
    pub legend: IndexMap<char, LazyRes<Pod>>,
}

impl SectorMap {
    pub fn from_area<'a, 'b>(
        r: &'a impl Environs,
        volume: &Cube,
        spawns: impl IntoIterator<Item = (&'b Location, &'b Pod)>,
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
            if !volume.contains(*loc) {
                continue;
            }

            let loc = ivec3(loc.x, loc.y, z);

            let name =
                idm::to_string(spawn).expect("Spawn serialization error");

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
                    crate::Tile::Surface(_, SplatteredStone) => '§',
                    crate::Tile::Surface(_, _) => '.',
                    crate::Tile::Wall(Door) => '+',
                    crate::Tile::Wall(Glass) => '|',
                    crate::Tile::Wall(Altar) => '=',
                    crate::Tile::Wall(Rubble) => '%',
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
            legend: rev_legend
                .into_iter()
                .map(|(k, v)| (v, LazyRes::new(k)))
                .collect(),
        }
    }

    pub fn upstairs() -> Self {
        SectorMap {
            map: "\
###
#<#
#.#"
            .to_owned(),
            ..Default::default()
        }
    }

    pub fn downstairs() -> Self {
        SectorMap {
            map: "\
#.#
#>#
#_#
###"
            .to_owned(),
            ..Default::default()
        }
    }

    pub fn entrances(&self) -> impl Iterator<Item = IVec2> + '_ {
        self.map
            .char_grid()
            .filter_map(|(p, c)| (c == '@').then_some(p))
    }

    pub fn find_downstairs(&self) -> Option<IVec2> {
        self.map
            .char_grid()
            .find_map(|(p, c)| (c == '>').then_some(p))
    }

    pub fn find_upstairs(&self) -> Option<IVec2> {
        self.map
            .char_grid()
            .find_map(|(p, c)| (c == '<').then_some(p))
    }

    pub fn dim(&self) -> IVec2 {
        self.map
            .char_grid()
            .map(|(p, _)| p)
            .fold(IVec2::ZERO, |a, x| a.max(x + ivec2(1, 1)))
    }

    pub fn spawns(
        &self,
        origin: Location,
    ) -> anyhow::Result<Vec<(Location, Pod)>> {
        let mut ret = Vec::default();

        for (p, c) in self.map.char_grid() {
            if let Some(name) = self.legend.get(&c) {
                ret.push((origin + p.extend(0), name.deref().clone()));
            }
        }

        Ok(ret)
    }

    pub fn border_and_inside(
        &self,
    ) -> (IndexMap<IVec2, char>, IndexMap<IVec2, char>) {
        let map: IndexMap<IVec2, char> = self.map.char_grid().collect();

        let mut border = IndexMap::default();
        let mut inside = IndexMap::default();

        for (p, c) in map.iter().map(|(&p, &c)| (p, c)).collect::<Vec<_>>() {
            if p.ns_8().all(|p| map.contains_key(&p)) {
                inside.insert(p, c);
            } else {
                border.insert(p, c);
            }
        }

        (border, inside)
    }

    pub fn terrain(&self, origin: Location) -> anyhow::Result<Cloud<3, Voxel>> {
        let mut ret = Cloud::default();

        for (p, c) in self.map.char_grid() {
            let p = origin + p.extend(0);

            let c = match c {
                // Rewrite entrace cells.
                '@' => '.',
                // Assume all spawns spawn on top of regular ground
                c if self.legend.contains_key(&c) => '.',
                c => c,
            };

            p.apply_char_terrain(&mut ret, c)?;
        }

        Ok(ret)
    }
}

impl Distribution<SectorMap> for StandardUniform {
    fn sample<R: rand::prelude::Rng + ?Sized>(&self, rng: &mut R) -> SectorMap {
        // Generate regular empty rectangular rooms.

        // Dimensions must be odd.
        const MAX_HALF_DIM: i32 = 6;
        let w = 2 * rng.random_range(2..MAX_HALF_DIM) + 1;
        let h = 2 * rng.random_range(2..MAX_HALF_DIM) + 1;

        let mut map = String::new();
        for y in 0..h {
            for x in 0..w {
                let on_v_edge = x == 0 || x == w - 1;
                let on_h_edge = y == 0 || y == h - 1;
                if on_v_edge && on_h_edge {
                    // Corner
                    map.push('#');
                } else if on_v_edge || on_h_edge {
                    // Edge
                    map.push('+');
                } else {
                    // Floor
                    map.push('.');
                }
            }
            if y < h - 1 {
                map.push('\n');
            }
        }

        SectorMap {
            map,
            ..Default::default()
        }
    }
}
