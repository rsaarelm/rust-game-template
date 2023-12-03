use glam::{ivec3, IVec3};
use serde::{Deserialize, Serialize};
use util::s4;

use crate::prelude::*;

#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Debug,
    Serialize,
    Deserialize,
)]
pub enum SectorDir {
    North,
    East,
    South,
    West,
    Up,
    Down,
}

impl From<SectorDir> for IVec3 {
    fn from(value: SectorDir) -> Self {
        use SectorDir::*;
        match value {
            North => ivec3(0, -SECTOR_HEIGHT, 0),
            East => ivec3(SECTOR_WIDTH, 0, 0),
            South => ivec3(0, SECTOR_HEIGHT, 0),
            West => ivec3(-SECTOR_WIDTH, 0, 0),
            Up => ivec3(0, 0, 1),
            Down => ivec3(0, 0, -1),
        }
    }
}

impl TryFrom<IVec2> for SectorDir {
    type Error = ();

    fn try_from(value: IVec2) -> Result<Self, Self::Error> {
        use SectorDir::*;

        let value = IVec2::ZERO.dir4_towards(&value);

        if value == IVec2::ZERO {
            Err(())
        } else {
            for (a, b) in s4::DIR.iter().zip([North, East, South, West]) {
                if *a == value {
                    return Ok(b);
                }
            }
            panic!("Bad IVec2 dir4_towards {:?}", value);
        }
    }
}

impl SectorDir {
    pub fn to_vec3(&self) -> IVec3 {
        match self {
            SectorDir::East => ivec3(SECTOR_WIDTH, 0, 0),
            SectorDir::South => ivec3(0, SECTOR_HEIGHT, 0),
            SectorDir::West => ivec3(-SECTOR_WIDTH, 0, 0),
            SectorDir::North => ivec3(0, -SECTOR_HEIGHT, 0),
            SectorDir::Up => ivec3(0, 0, 1),
            SectorDir::Down => ivec3(0, 0, -1),
        }
    }
}
