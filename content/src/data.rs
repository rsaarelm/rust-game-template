use std::{
    collections::BTreeMap, fmt, path::Path, str::FromStr, sync::OnceLock,
};

use anyhow::bail;
use derive_more::{Deref, From};
use glam::IVec2;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
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
    pub loadout: LazyRes<Pod>,
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

/// A pod is an inert value that can hatch into one or several live runtime
/// objects.
#[derive(
    Clone,
    Debug,
    Default,
    Deref,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    DeserializeFromStr,
    Serialize,
)]
pub struct Pod(Vec<((PodObject,), Pod)>);

impl<'a> IntoIterator for &'a Pod {
    type Item = &'a ((PodObject,), Pod);

    type IntoIter = std::slice::Iter<'a, ((PodObject,), Pod)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromStr for Pod {
    type Err = idm::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // XXX Need to hack around IDM here so newline-less single line items
        // can still work as pods.
        if s.trim().is_empty() {
            Ok(Default::default())
        } else if !s.chars().any(|c| c == '\n') {
            // Parse the insides using standard IDM routine, be sure to wrap
            // it in Pod outside of the IDM parse so that we don't just get an
            // infinite recursion to the from_str wrapper.
            Ok(Pod(idm::from_str(&format!("{s}\n"))?))
        } else {
            Ok(Pod(idm::from_str(s)?))
        }
    }
}

impl<T: Into<PodObject>> From<T> for Pod {
    fn from(value: T) -> Self {
        Pod(vec![((value.into(),), Pod(vec![]))])
    }
}

/// A single element in a hatch specification, object contents are not stored
/// in eggs.
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    DeserializeFromStr,
    SerializeDisplay,
)]
pub struct PodObject {
    /// How many copies of this object are hatched?
    ///
    /// Stackable objects will form a single stack, non-stackable objects will
    /// appear in the same place, being offset from earlier hatched objects as
    /// needed.
    pub count: i32,
    /// The name of the object, this isn't stored in `PodKind` data.
    pub name: String,
    /// What kind of an object it is, concrete properties.
    pub kind: PodKind,
}

impl fmt::Display for PodObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.count != 1 {
            write!(f, "{}x ", self.count)?;
        }

        write!(f, "{}", self.name)
    }
}

impl PodObject {
    pub fn new(name: String, kind: PodKind) -> Self {
        PodObject {
            count: 1,
            name,
            kind,
        }
    }

    /// Set the element count of the egg to something other than 1.
    pub fn x(mut self, count: i32) -> Self {
        assert!(count > 0);
        self.count = count;
        self
    }
}

impl FromStr for PodObject {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (count, name) = util::parse::multipliable(s);
        let kind = name.parse()?;
        let name = name.to_string();

        Ok(PodObject { count, name, kind })
    }
}

/// The concrete data of an object to hatch, what kind of thing is it.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, From)]
pub enum PodKind {
    #[from]
    Monster(&'static Monster),
    #[from]
    Item(&'static Item),
}

impl FromStr for PodKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Magic switchboard that trawls the data files looking for named
        // things that can be hatched.
        if let Some(monster) = Data::get().bestiary.get(s) {
            return Ok(PodKind::Monster(monster));
        }

        if let Some(item) = Data::get().armory.get(s) {
            return Ok(PodKind::Item(item));
        }

        bail!("Unknown pod kind {s:?}")
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

impl SpawnDist for PodKind {
    fn rarity(&self) -> u32 {
        match self {
            PodKind::Monster(a) => a.rarity(),
            PodKind::Item(a) => a.rarity(),
        }
    }

    fn min_depth(&self) -> u32 {
        match self {
            PodKind::Monster(a) => a.min_depth(),
            PodKind::Item(a) => a.min_depth(),
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
    Summon(LazyRes<PodObject>),
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
