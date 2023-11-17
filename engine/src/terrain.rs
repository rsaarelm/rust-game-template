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
                '_' => None,
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
            None => (loc, '_'),
            Some(b) => (loc, char::from(*b)),
        }))
    }
}
