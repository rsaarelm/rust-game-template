use derive_more::{Deref, DerefMut};
use serde::{Deserialize, Serialize};

use crate::{prelude::*, Atlas};

#[derive(Clone, Default, Deref, DerefMut, Serialize, Deserialize)]
#[serde(try_from = "Atlas", into = "Atlas")]
pub struct VoxelTerrain(HashMap<Location, Voxel>);

impl TryFrom<Atlas> for VoxelTerrain {
    type Error = anyhow::Error;

    fn try_from(value: Atlas) -> Result<Self, Self::Error> {
        let mut ret = VoxelTerrain::default();

        for (loc, c) in value.iter() {
            let v = match c {
                '.' => None,
                c => Some(Block::try_from(c)?),
            };
            ret.insert(loc, v);
        }
        Ok(ret)
    }
}

impl From<VoxelTerrain> for Atlas {
    fn from(map: VoxelTerrain) -> Self {
        Atlas::from_iter(map.0.iter().map(|(&loc, v)| match v {
            None => (loc, '.'),
            Some(b) => (loc, char::from(*b)),
        }))
    }
}

/// Game world terrain tiles.
#[derive(Clone, Default, Deref, DerefMut, Serialize, Deserialize)]
#[serde(try_from = "Atlas", into = "Atlas")]
#[deprecated]
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

#[derive(Clone, Default, Deref, DerefMut, Serialize, Deserialize)]
#[serde(try_from = "Atlas", into = "Atlas")]
pub struct Terrain(HashMap<Location, Option<Block>>);

impl TryFrom<Atlas> for Terrain {
    type Error = anyhow::Error;

    fn try_from(value: Atlas) -> Result<Self, Self::Error> {
        let mut ret = Terrain::default();

        for (loc, c) in value.iter() {
            let voxel = if c == '.' {
                None
            } else {
                Some(Block::try_from(c)?)
            };

            ret.insert(loc, voxel);
        }
        Ok(ret)
    }
}

impl From<Terrain> for Atlas {
    fn from(map: Terrain) -> Self {
        Atlas::from_iter(map.0.into_iter().map(|(p, v)| match v {
            None => (p, '.'),
            Some(b) => (p, b.into()),
        }))
    }
}
