use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use strum::EnumIter;
use util::{IncrementalOutline, IndexMap, Outline, _String};

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

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Monster {
    pub icon: char,
    pub might: i32,
    pub rarity: u32,
    pub min_depth: u32,
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
