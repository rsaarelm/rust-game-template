use serde::{Deserialize, Serialize};

/// Specific terrain in a single game world map cell.
#[derive(
    Copy, Clone, Default, Eq, PartialEq, Debug, Serialize, Deserialize,
)]
#[serde(try_from = "char", into = "char")]
pub enum MapTile {
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

use MapTile::*;

impl MapTile {
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
    /// use engine::MapTile;
    ///
    /// assert_eq!(MapTile::Water.mix(MapTile::Magma), MapTile::Magma);
    /// assert_eq!(MapTile::Grass.mix(MapTile::Ground), MapTile::Ground);
    /// ```
    pub fn mix(self, other: MapTile) -> Self {
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

impl TryFrom<char> for MapTile {
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

impl From<MapTile> for char {
    fn from(val: MapTile) -> Self {
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
