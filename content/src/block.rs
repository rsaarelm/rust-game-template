use anyhow::bail;
use derive_more::{Deref, DerefMut};
use serde::{Deserialize, Serialize};
use util::Cloud;

use crate::{Atlas, Location};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
// 2D representation for a top-down view of voxel location.
pub enum Tile {
    /// Surface with empty space above it.
    ///
    /// May be displaced from slice center by +/-1 z.
    Surface(Location, Block),

    /// Solid vertical mass.
    Wall(Block),

    /// Empty space with no visible floor.
    Void,
}

impl Tile {
    pub fn is_wall(&self) -> bool {
        matches!(self, Tile::Wall(_))
    }
}

pub type Voxel = Option<Block>;

/// Possible contents for a voxel.
#[derive(
    Copy, Clone, Default, Eq, PartialEq, Hash, Debug, Serialize, Deserialize,
)]
pub enum Block {
    #[default]
    /// Smooth, worked walls, shown as shaped walls.
    Stone,
    SplatteredRock,
    Grass,
    Glass,
    /// Rough, unworked mass, drawn as undifferentiated blob.
    Rubble,

    Altar,
    Door,

    Water,
    Magma,
}

use Block::*;

impl Block {
    /// Block is solid matter that can be stood on top of.
    pub fn is_support(self) -> bool {
        matches!(self, Stone | SplatteredRock | Rubble | Grass | Glass)
    }

    pub fn blocks_sight(self) -> bool {
        matches!(self, Stone | SplatteredRock | Rubble | Grass | Magma | Door)
    }
}

// NB. Char '_' is reserved for "empty space", don't use it for any block

impl TryFrom<char> for Block {
    type Error = anyhow::Error;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '*' => Ok(Stone),
            '§' => Ok(SplatteredRock),
            ';' => Ok(Grass),
            '|' => Ok(Glass),
            '%' => Ok(Rubble),

            '=' => Ok(Altar),
            '+' => Ok(Door),

            '~' => Ok(Water),
            '&' => Ok(Magma),
            _ => bail!("Bad block {value:?}"),
        }
    }
}

impl From<Block> for char {
    fn from(value: Block) -> Self {
        // This must match the mapping in Block::try_from.
        match value {
            Stone => '*',
            SplatteredRock => '§',
            Grass => ';',
            Glass => '|',
            Rubble => '%',

            Altar => '=',
            Door => '+',

            Water => '~',
            Magma => '&',
        }
    }
}

/// Voxel collection patches that serialize nicely.
#[derive(Clone, Default, Deref, DerefMut, Debug, Serialize, Deserialize)]
#[serde(try_from = "Atlas", into = "Atlas")]
pub struct Terrain(Cloud<3, Voxel>);

impl TryFrom<Atlas> for Terrain {
    type Error = anyhow::Error;

    fn try_from(value: Atlas) -> Result<Self, Self::Error> {
        let mut ret = Terrain::default();

        for (loc, c) in value.iter() {
            if c == '_' {
                ret.0.insert(loc, None);
            } else {
                ret.0.insert(loc, Some(Block::try_from(c)?));
            }
        }
        Ok(ret)
    }
}

impl From<Terrain> for Atlas {
    fn from(map: Terrain) -> Self {
        Atlas::from_iter(map.0.into_iter().map(|(loc, v)| match v {
            None => (loc, '_'),
            Some(b) => (loc, b.into()),
        }))
    }
}
