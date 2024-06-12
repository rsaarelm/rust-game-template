use std::{
    collections::BTreeMap, fmt, path::Path, str::FromStr, sync::OnceLock,
};

use anyhow::bail;
use glam::IVec2;
use serde::{Deserialize, Serialize};
use strum::EnumIter;
use util::{HashMap, IndexMap, LazyRes, _String};

use crate::SectorMap;

static DATA: OnceLock<Data> = OnceLock::new();

/// Load content data from filesystem path.
pub fn register_data_from(path: impl AsRef<Path>) -> anyhow::Result<()> {
    let data = util::dir_to_idm(path.as_ref())?;
    register_data(idm::from_str(&data.to_string()).unwrap());
    Ok(())
}

/// Register content data directly from value.
pub fn register_data(data: Data) {
    match DATA.get() {
        None => {
            let _ = DATA.set(data);
        }
        Some(x) if x == &data => {
            log::info!("registering the same gamedata twice, ignored");
        }
        _ => {
            panic!("Tried to register different gamedata when data is already registered");
        }
    }
}

/// Static global game data.
#[derive(Clone, Default, Eq, PartialEq, Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Data {
    pub settings: Settings,
    pub bestiary: IndexMap<_String, Monster>,
    pub armory: IndexMap<_String, Item>,
    pub campaign: BTreeMap<String, Scenario>,
    /// Irregular plural words.
    pub plurals: HashMap<String, String>,
}

/// Game-wide general settings.
#[derive(Clone, Default, Eq, PartialEq, Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Settings {
    /// Internal human-readable identifier of the game, used for save
    /// directories etc.
    pub id: String,
    /// Player-visible full title of the game.
    pub title: String,
}

pub fn settings() -> &'static Settings {
    &Data::get().settings
}

// Custom loader that initializes the global static gamedata from the data
// files. The data.idm.sz file is constructed from project data files by engine
// crate's build.rs script.
impl Default for &'static Data {
    fn default() -> Self {
        DATA.get().expect("No data registered")
    }
}

impl Data {
    pub fn get() -> &'static Data {
        Default::default()
    }
}

#[derive(
    Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize,
)]
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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Scenario {
    pub map: String,
    pub legend: IndexMap<char, Vec<Region>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
        match self {
            Region::Site(_) => true,
            Region::Repeat(_, r) => r.is_site(),
            _ => false,
        }
    }

    pub fn is_prefab(&self) -> bool {
        match self {
            Region::Site(_) | Region::Hall(_) => true,
            Region::Repeat(_, a) => a.is_prefab(),
            _ => false,
        }
    }

    pub fn height(&self) -> i32 {
        match self {
            Region::Repeat(n, a) => *n as i32 * a.height(),
            Region::Branch(_) => 0,
            _ => 1,
        }
    }

    /// How many vertical floors this region represents.
    pub fn count(&self) -> u32 {
        match self {
            Region::Repeat(n, inner) => n * inner.count(),
            // Branches go off to the side so they don't add to count.
            Region::Branch(_) => 0,
            _ => 1,
        }
    }

    pub fn fixed_upstairs(&self) -> Option<IVec2> {
        match self {
            Region::Site(a) | Region::Hall(a) => a.find_upstairs(),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GenericSector {
    Water,
    Grassland,
    Forest,
    Mountains,
    Dungeon,
}

#[derive(
    Clone, Default, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize,
)]
#[serde(default, rename_all = "kebab-case")]
pub struct Monster {
    pub icon: char,
    pub might: i32,
    pub evasion: i32,
    pub attack_damage: i32,
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

#[derive(
    Clone, Default, Eq, PartialEq, Ord, PartialOrd, Debug, Deserialize,
)]
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
    Copy,
    Clone,
    Default,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Serialize,
    Deserialize,
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

    pub fn is_stacking(&self) -> bool {
        use ItemKind::*;
        matches!(self, Scroll | Potion | Treasure)
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
    Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Power {
    CallLightning,
    Confusion,
    Fireball,
    MagicMapping,
    HealSelf,
    Summon(LazyRes<_String, Spawn>),
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
        register_data_from("../data").unwrap();
        assert!(!Data::get().bestiary.is_empty());
    }
}
