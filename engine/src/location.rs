use glam::{ivec3, IVec3};
use serde::{Deserialize, Serialize};
use util::s4;

use crate::prelude::*;

/// Absolute locations in the game world.
#[derive(
    Copy,
    Clone,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Debug,
    Serialize,
    Deserialize,
)]
// Serialize as tuples so that locations are inlineable in IDM saves.
#[serde(from = "(i16, i16, i16)", into = "(i16, i16, i16)")]
pub struct Location {
    // NB. Fields are in this order so that lexical sorting produces a
    // "natural" order of layers, then rows, then columns.
    pub(crate) z: i16,
    pub(crate) y: i16,
    pub(crate) x: i16,
}

impl Location {
    pub fn new(x: i16, y: i16, z: i16) -> Self {
        Location { x, y, z }
    }
}

impl From<IVec3> for Location {
    fn from(value: IVec3) -> Self {
        Location::new(value.x as i16, value.y as i16, value.z as i16)
    }
}

impl From<(i16, i16, i16)> for Location {
    fn from((x, y, z): (i16, i16, i16)) -> Self {
        Location { x, y, z }
    }
}

impl From<Location> for (i16, i16, i16) {
    fn from(val: Location) -> Self {
        (val.x, val.y, val.z)
    }
}

impl From<Location> for IVec2 {
    fn from(val: Location) -> Self {
        val.unfold()
    }
}

impl From<Location> for IVec3 {
    fn from(val: Location) -> Self {
        val.to_vec3()
    }
}

impl std::ops::Sub<Location> for Location {
    type Output = IVec3;

    fn sub(self, rhs: Location) -> Self::Output {
        Into::<IVec3>::into(self) - Into::<IVec3>::into(rhs)
    }
}

impl std::ops::Add<IVec2> for Location {
    type Output = Location;

    fn add(self, rhs: IVec2) -> Self::Output {
        let mut ret = self;
        ret += rhs;
        ret
    }
}

impl std::ops::AddAssign<IVec2> for Location {
    fn add_assign(&mut self, rhs: IVec2) {
        self.x += rhs.x as i16;
        self.y += rhs.y as i16;
    }
}

impl std::ops::Sub<IVec2> for Location {
    type Output = Location;

    fn sub(self, rhs: IVec2) -> Self::Output {
        let mut ret = self;
        ret -= rhs;
        ret
    }
}

impl std::ops::SubAssign<IVec2> for Location {
    fn sub_assign(&mut self, rhs: IVec2) {
        self.x -= rhs.x as i16;
        self.y -= rhs.y as i16;
    }
}

impl std::ops::Add<IVec3> for Location {
    type Output = Location;

    fn add(self, rhs: IVec3) -> Self::Output {
        let mut ret = self;
        ret += rhs;
        ret
    }
}

impl std::ops::AddAssign<IVec3> for Location {
    fn add_assign(&mut self, rhs: IVec3) {
        self.x += rhs.x as i16;
        self.y += rhs.y as i16;
        self.z += rhs.z as i16;
    }
}

impl std::ops::Sub<IVec3> for Location {
    type Output = Location;

    fn sub(self, rhs: IVec3) -> Self::Output {
        let mut ret = self;
        ret -= rhs;
        ret
    }
}

impl std::ops::SubAssign<IVec3> for Location {
    fn sub_assign(&mut self, rhs: IVec3) {
        self.x -= rhs.x as i16;
        self.y -= rhs.y as i16;
        self.z -= rhs.z as i16;
    }
}

impl std::ops::Add<SectorDir> for Location {
    type Output = Location;

    fn add(self, rhs: SectorDir) -> Self::Output {
        let mut ret = self;
        ret += rhs;
        ret
    }
}

impl std::ops::AddAssign<SectorDir> for Location {
    fn add_assign(&mut self, rhs: SectorDir) {
        *self += rhs.to_vec3();
    }
}

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

#[cfg(test)]
mod test {
    use super::*;
    use quickcheck::{Arbitrary, Gen};
    use quickcheck_macros::quickcheck;

    #[test]
    fn sector() {
        assert_eq!(Location::default().sector(), Location::default());
        assert_eq!(
            Location::new(SECTOR_WIDTH as i16 / 2, SECTOR_HEIGHT as i16 / 2, 0)
                .sector(),
            Location::default()
        );

        assert_eq!(
            Location::new(-1, -1, 0).sector(),
            Location::new(-SECTOR_WIDTH as i16, -SECTOR_HEIGHT as i16, 0)
        );

        assert_eq!(
            Location::new(0, 0, 0).sector_dist(&Location::new(-1, 0, 0)),
            1
        );

        assert_eq!(
            Location::new(0, 0, 0).sector_dist(&Location::new(-1, -1, 0)),
            2
        );
    }

    impl Arbitrary for Location {
        fn arbitrary(g: &mut Gen) -> Location {
            Location {
                x: i16::arbitrary(g),
                y: i16::arbitrary(g),
                z: i16::arbitrary(g),
            }
        }
    }

    #[quickcheck]
    fn location_to_ivec2(loc: Location) -> bool {
        let vec: IVec2 = loc.into();
        Location::fold(vec) == loc
    }
}
