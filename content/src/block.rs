use anyhow::bail;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
// 2D representation for a top-down view of voxel location.
pub enum Tile {
    /// Floor seen from above.
    Floor {
        block: Block,

        /// Relative depth. 0 is the current observation level.
        z: i32,

        /// Slope connectivity to unseen tiles above at z=1 and unseen tiles
        /// below at z=-1. Should always be 0 at z=0.
        connectivity: usize,
    },

    /// Inside a solid mass, not visible from any direction.
    Solid(Block),

    /// Empty void,
    Void,
}

impl Tile {
    pub fn is_wall(&self) -> bool {
        matches!(self, Tile::Solid(_))
    }
}

pub type Voxel = Option<Block>;

/// Possible contents for a voxel.
#[derive(
    Copy, Clone, Default, Eq, PartialEq, Hash, Debug, Serialize, Deserialize,
)]
pub enum Block {
    #[default]
    Rock,
    Grass,
    Door,
    Glass,
    Water,
    Magma,
}

use Block::*;

impl Block {
    pub fn is_solid(self) -> bool {
        matches!(self, Rock | Grass | Glass)
    }

    pub fn blocks_sight(self) -> bool {
        matches!(self, Rock | Grass | Magma | Door)
    }
}

// NB. Char '_' is reserved for "empty space", don't use it for any block

impl TryFrom<char> for Block {
    type Error = anyhow::Error;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '*' => Ok(Rock),
            ';' => Ok(Grass),
            '|' => Ok(Door),
            '+' => Ok(Glass),
            '~' => Ok(Water),
            '&' => Ok(Magma),
            _ => bail!("Bad block {value:?}"),
        }
    }
}

impl From<Block> for char {
    fn from(value: Block) -> Self {
        match value {
            Rock => '*',
            Grass => ';',
            Door => '|',
            Glass => '+',
            Water => '~',
            Magma => '&',
        }
    }
}
