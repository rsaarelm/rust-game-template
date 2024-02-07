use std::{fmt, str::FromStr, sync::OnceLock};

use anyhow::bail;
use glam::IVec2;
use serde::{Deserialize, Serialize};
use strum::EnumIter;
use util::{text, IncrementalOutline, IndexMap, Outline, _String};

use crate::SectorMap;

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
// files. The data.idm.sz file is constructed from project data files by engine
// crate's build.rs script.
impl Default for &'static Data {
    fn default() -> Self {
        DATA.get_or_init(|| {
            let data = snap::raw::Decoder::new()
                .decompress_vec(include_bytes!("../../target/data.idm.sz"))
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
    /// Sites are always above ground, though some generated regions may be
    /// above ground too.
    pub fn is_site(&self) -> bool {
        matches!(self, Region::Site(_))
    }

    /// If the region specifies a concrete prefab map, return the map.
    pub fn as_map(&self) -> Option<&SectorMap> {
        match self {
            Region::Site(map) => Some(map),
            Region::Hall(map) => Some(map),
            Region::Repeat(_, n) => n.as_map(),
            _ => None,
        }
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
