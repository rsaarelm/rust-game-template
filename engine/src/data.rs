use std::sync::LazyLock;

use serde::Deserialize;
use util::{IndexMap, UnderscoreString};

use crate::item::ItemKind;

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Data {
    pub bestiary: IndexMap<UnderscoreString, Monster>,
    pub armory: IndexMap<UnderscoreString, Item>,
}

// Custom loader that initializes the global static gamedata from the data
// files. The data.idm.z file is constructed from project data files by engine
// crate's build.rs script.
impl Default for &'static Data {
    fn default() -> Self {
        static DATA: LazyLock<Data> = LazyLock::new(|| {
            let data = fdeflate::decompress_to_vec(include_bytes!(
                "../../target/data.idm.z"
            ))
            .unwrap();
            let data = std::str::from_utf8(&data).unwrap();
            let data: Data = idm::from_str(data).unwrap();

            data
        });

        &DATA
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
    pub power: i32,
    pub min_depth: i32,
}

#[derive(Clone, Default, Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Item {
    pub power: i32,
    pub kind: ItemKind,
}
