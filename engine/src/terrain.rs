use derive_deref::{Deref, DerefMut};
use serde::{Deserialize, Serialize};

use crate::{prelude::*, Atlas};

/// Game world terrain tiles.
#[derive(Clone, Default, Deref, DerefMut, Serialize, Deserialize)]
#[serde(try_from = "Atlas", into = "Atlas")]
pub struct TileTerrain(HashMap<Location, MapTile>);

impl TryFrom<Atlas> for TileTerrain {
    type Error = &'static str;

    fn try_from(value: Atlas) -> Result<Self, Self::Error> {
        let mut ret = TileTerrain::default();

        for (loc, c) in value.iter() {
            let c = MapTile::try_from(c)?;
            if c != Default::default() {
                ret.insert(loc, c);
            }
        }
        Ok(ret)
    }
}

impl From<TileTerrain> for Atlas {
    fn from(map: TileTerrain) -> Self {
        Atlas::from_iter(map.0)
    }
}
