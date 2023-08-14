use serde::{Deserialize, Serialize};

/// Specific terrain in a single game world map cell.
#[derive(
    Copy, Clone, Default, Eq, PartialEq, Debug, Serialize, Deserialize,
)]
#[serde(try_from = "char", into = "char")]
pub enum Tile {
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

use Tile::*;

impl Tile {
    pub fn blocks_sight(self) -> bool {
        matches!(self, Wall | Door)
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

    /// Return the type of terrain that should show up in between two types of
    /// terrain on a double-width map.
    ///
    /// ```
    /// use engine::Tile;
    ///
    /// assert_eq!(Tile::Water.mix(Tile::Magma), Tile::Magma);
    /// assert_eq!(Tile::Grass.mix(Tile::Ground), Tile::Ground);
    /// ```
    pub fn mix(self, other: Tile) -> Self {
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

impl TryFrom<char> for Tile {
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

impl Into<char> for Tile {
    fn into(self) -> char {
        // NB. This must match Tile's TryFrom inputs above.
        match self {
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
