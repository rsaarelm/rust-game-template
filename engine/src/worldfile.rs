use serde::Deserialize;

use crate::{prelude::*, Atlas, Prototypes, Quest};

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Worldfile {
    pub quest: Option<Quest>,
    pub terrain: Atlas,
    pub legend: IndexMap<char, String>,
    pub lexicon: Prototypes,
}
