use serde::Deserialize;

use crate::{prelude::*, Atlas, Prototypes};

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Worldfile {
    pub terrain: Atlas,
    pub legend: IndexMap<char, String>,
    pub lexicon: Prototypes,
}
