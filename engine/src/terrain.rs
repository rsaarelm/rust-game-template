use derive_deref::{Deref, DerefMut};
use serde::{Deserialize, Serialize};

use crate::{prelude::*, Atlas};

/// Game world terrain tiles.
#[derive(Clone, Default, Deref, DerefMut, Serialize, Deserialize)]
#[serde(try_from = "Atlas", into = "Atlas")]
pub struct Terrain(HashMap<Location, Tile>);

impl TryFrom<Atlas> for Terrain {
    type Error = &'static str;

    fn try_from(value: Atlas) -> Result<Self, Self::Error> {
        let mut ret = Terrain::default();

        for (loc, c) in value.iter() {
            let c = Tile::try_from(c)?;
            if c != Default::default() {
                ret.insert(loc, c);
            }
        }
        Ok(ret)
    }
}

impl From<Terrain> for Atlas {
    fn from(map: Terrain) -> Self {
        Atlas::from_iter(map.0)
    }
}
