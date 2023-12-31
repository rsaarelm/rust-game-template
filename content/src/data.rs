use std::{fmt, str::FromStr, sync::OnceLock};

use anyhow::bail;
use glam::{ivec3, IVec2};
use serde::{Deserialize, Serialize};
use strum::EnumIter;
use util::{
    IncrementalOutline, IndexMap, Outline, _String, text, Cloud, HashMap,
};

use crate::{Block, Coordinates, Cube, Environs, Location, Voxel};

static DATA: OnceLock<Data> = OnceLock::new();

static MODS: OnceLock<Vec<IncrementalOutline>> = OnceLock::new();

pub fn register_mods(mods: Vec<IncrementalOutline>) {
    assert!(
        DATA.get().is_none(),
        "too late to register mods, game data is already initialized"
    );
    assert!(MODS.get().is_none(), "mods can only be registered once");
    MODS.set(mods).unwrap();
}

/// Static global game data.
#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Data {
    pub bestiary: IndexMap<_String, Monster>,
    pub armory: IndexMap<_String, Item>,
    pub scenario: Scenario,
}

// Custom loader that initializes the global static gamedata from the data
// files. The data.idm.z file is constructed from project data files by engine
// crate's build.rs script.
impl Default for &'static Data {
    fn default() -> Self {
        DATA.get_or_init(|| {
            let data = fdeflate::decompress_to_vec(include_bytes!(
                "../../target/data.idm.z"
            ))
            .unwrap();
            let data = std::str::from_utf8(&data).unwrap();
            let mut data: Outline = idm::from_str(data).unwrap();

            let mods: &Vec<IncrementalOutline> =
                MODS.get_or_init(Default::default);

            for md in mods {
                data += md;
            }

            idm::transmute(&data).unwrap()
        })
    }
}

impl Data {
    pub fn get() -> &'static Data {
        Default::default()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "_String", into = "_String")]
pub enum Spawn {
    Monster(String, &'static Monster),
    Item(String, &'static Item),
}

impl TryFrom<_String> for Spawn {
    type Error = anyhow::Error;

    fn try_from(value: _String) -> Result<Self, Self::Error> {
        (*value).parse()
    }
}

impl From<Spawn> for _String {
    fn from(value: Spawn) -> Self {
        _String(value.to_string())
    }
}

impl FromStr for Spawn {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Magic switchboard that trawls the data files looking for named
        // things that can be spawned.
        if let Some(monster) = Data::get().bestiary.get(s) {
            return Ok(Spawn::Monster(s.into(), monster));
        }

        if let Some(item) = Data::get().armory.get(s) {
            return Ok(Spawn::Item(s.into(), item));
        }

        bail!("Unknown spawn {s:?}")
    }
}

impl fmt::Display for Spawn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Spawn::Monster(name, _) | Spawn::Item(name, _) => {
                write!(f, "{name}")
            }
        }
    }
}

pub trait SpawnDist {
    fn rarity(&self) -> u32;
    fn min_depth(&self) -> u32;

    fn spawn_weight(&self) -> f64 {
        match self.rarity() {
            0 => 0.0,
            r => 1.0 / r as f64,
        }
    }
}

impl SpawnDist for Spawn {
    fn rarity(&self) -> u32 {
        match self {
            Spawn::Monster(_, a) => a.rarity(),
            Spawn::Item(_, a) => a.rarity(),
        }
    }

    fn min_depth(&self) -> u32 {
        match self {
            Spawn::Monster(_, a) => a.min_depth(),
            Spawn::Item(_, a) => a.min_depth(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Scenario {
    pub map: String,
    pub legend: IndexMap<char, Vec<Region>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Region {
    /// A procgen level
    Generate(GenericSector),
    /// An above-ground prefab level
    Site(SectorMap),
    /// An underground prefab level
    Hall(SectorMap),
    /// Branch a new stack off to the side
    Branch(Vec<Region>),
    /// A sequence of applying the same constructor multiple times.
    Repeat(u32, Box<Region>),
}

impl Region {
    pub fn is_above_ground(&self) -> bool {
        matches!(self, Region::Site(_))
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GenericSector {
    Water,
    Grassland,
    Forest,
    Mountains,
    Dungeon,
}

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

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Monster {
    pub icon: char,
    pub might: i32,
    pub rarity: u32,
    pub min_depth: u32,
}

impl SpawnDist for Monster {
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

impl SpawnDist for Item {
    fn rarity(&self) -> u32 {
        self.rarity
    }

    fn min_depth(&self) -> u32 {
        0
    }
}

#[derive(
    Copy, Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum ItemKind {
    // Have a baked-in None value so this can be used directly as a component
    #[default]
    None,
    MeleeWeapon,
    RangedWeapon,
    Armor,
    Ring,
    Scroll,
    Potion,
    Treasure,
}

impl ItemKind {
    pub fn fits(&self, slot: EquippedAt) -> bool {
        use EquippedAt::*;
        use ItemKind::*;
        match self {
            MeleeWeapon => slot == RunHand,
            RangedWeapon => slot == RunHand || slot == GunHand,
            Armor => slot == Body,
            Ring => slot == Ring1 || slot == Ring2,
            _ => false,
        }
    }

    pub fn icon(&self) -> char {
        use ItemKind::*;
        match self {
            None => 'X',
            MeleeWeapon => ')',
            RangedWeapon => ')',
            Armor => '[',
            Ring => '°',
            Scroll => '?',
            Potion => '!',
            Treasure => '$',
        }
    }
}

#[derive(
    Copy, Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize, EnumIter,
)]
#[serde(rename_all = "kebab-case")]
pub enum EquippedAt {
    #[default]
    None,
    RunHand,
    GunHand,
    Body,
    Ring1,
    Ring2,
}

impl EquippedAt {
    pub fn is_none(&self) -> bool {
        matches!(self, EquippedAt::None)
    }

    pub fn is_some(&self) -> bool {
        !self.is_none()
    }
}

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Power {
    BerserkRage,
    CallLightning,
    Confusion,
    Fireball,
    MagicMapping,
    HealSelf,
}

impl Power {
    pub fn needs_aim(self) -> bool {
        use Power::*;
        matches!(self, Confusion | Fireball)
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
