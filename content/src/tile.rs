use derive_more::{Deref, DerefMut};
use serde::{Deserialize, Serialize};
use util::Cloud;

use crate::Atlas;

/// Specific terrain in a single game world map cell.
#[derive(
    Copy, Clone, Default, Eq, PartialEq, Debug, Serialize, Deserialize,
)]
#[serde(try_from = "char", into = "char")]
pub enum Tile2D {
    #[default]
    Wall,
    Ground,
    Grass,
    LowWall,
    Door,
    Water,
    Magma,
    Upstairs,
    Downstairs,
    Gore,
    Exit,
}

use Tile2D::*;

impl Tile2D {
    pub fn blocks_sight(self) -> bool {
        matches!(self, Wall | Door)
    }

    pub fn is_walkable(self) -> bool {
        !self.blocks_movement()
    }

    pub fn blocks_movement(self) -> bool {
        matches!(self, Wall | LowWall | Water | Magma)
    }

    pub fn blocks_shot(self) -> bool {
        matches!(self, Wall | Door | Upstairs | Downstairs)
    }

    pub fn is_wall(self) -> bool {
        matches!(self, Wall | LowWall | Door)
    }

    pub fn is_exit(self) -> bool {
        matches!(self, Upstairs | Downstairs)
    }

    pub fn is_decoration(self) -> bool {
        matches!(self, Gore)
    }

    /// Other wall edge height for purposes of shaped wall display.
    pub fn edge_height(self) -> usize {
        match self {
            Wall => 2,
            LowWall => 1,
            // Doors open into doorways that have zero wall height.
            Door => 0,
            _ => 0,
        }
    }

    /// Self height for purposes of shaped wall display
    pub fn self_height(self) -> usize {
        match self {
            Wall | Door => 2,
            LowWall => 1,
            // Doors open into doorways that have zero wall height.
            _ => 0,
        }
    }

    /// Return the type of terrain that should show up in between two types of
    /// terrain on a double-width map.
    ///
    /// ```
    /// use content::Tile2D;
    ///
    /// assert_eq!(Tile2D::Water.mix(Tile2D::Magma), Tile2D::Magma);
    /// assert_eq!(Tile2D::Grass.mix(Tile2D::Ground), Tile2D::Ground);
    /// ```
    pub fn mix(self, other: Tile2D) -> Self {
        match (self, other) {
            (x, y) if x == y => x,
            // All defined matches are an earlier terrain type matched against
            // a later one, flip the inverse cases.
            (x, y) if x as usize > y as usize => y.mix(x),
            (Ground | Grass | Exit, Water) => Water,
            (Ground | Grass | Exit, Magma) => Magma,
            (Wall | Door, Water) => Water,
            (Wall | Door, Magma) => Magma,
            (Wall, LowWall) => LowWall,
            (Wall, Door) => Wall,
            (LowWall, Door) => LowWall,
            (Water, Magma) => Magma,

            _ => Ground,
        }
    }
}

impl TryFrom<char> for Tile2D {
    type Error = &'static str;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '.' => Ok(Ground),
            '#' => Ok(Wall),
            '-' => Ok(LowWall),
            '+' => Ok(Door),
            ',' => Ok(Grass),
            '~' => Ok(Water),
            '&' => Ok(Magma),
            '<' => Ok(Upstairs),
            '>' => Ok(Downstairs),
            '§' => Ok(Gore),
            'X' => Ok(Exit),
            _ => Err("invalid terrain char"),
        }
    }
}

impl From<Tile2D> for char {
    fn from(val: Tile2D) -> Self {
        // NB. This must match Tile's TryFrom inputs above.
        match val {
            Ground => '.',
            Wall => '#',
            LowWall => '-',
            Door => '+',
            Grass => ',',
            Water => '~',
            Magma => '&',
            Upstairs => '<',
            Downstairs => '>',
            Gore => '§',
            Exit => 'X',
        }
    }
}

/// Game world terrain tiles.
#[derive(Clone, Default, Deref, DerefMut, Serialize, Deserialize)]
#[serde(try_from = "Atlas", into = "Atlas")]
pub struct Terrain(Cloud<3, Tile2D>);

impl TryFrom<Atlas> for Terrain {
    type Error = &'static str;

    fn try_from(value: Atlas) -> Result<Self, Self::Error> {
        let mut ret = Terrain::default();

        for (loc, c) in value.iter() {
            let c = Tile2D::try_from(c)?;
            if c != Default::default() {
                ret.0.insert(loc, c);
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
