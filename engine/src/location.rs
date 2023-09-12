use glam::{ivec3, IVec3};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use util::s4;

use crate::{prelude::*, Grammatize, Rect, SECTOR_HEIGHT, SECTOR_WIDTH};

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
    z: i16,
    y: i16,
    x: i16,
}

impl Location {
    pub fn new(x: i16, y: i16, z: i16) -> Self {
        Location { x, y, z }
    }
}

impl Location {
    pub fn z(&self) -> i16 {
        self.z
    }

    /// Convert to 2D vector, layering Z-levels vertically in 2-plane.
    ///
    /// Each location has a unique point on the `IVec2` plane and the original
    /// location can be retrieved by calling `Location::from` on the `IVec2`
    /// value.
    pub fn unfold(&self) -> IVec2 {
        // Maps y: i16::MIN, z: i16::MIN to i32::MIN.
        let y = self.y as i64 + self.z as i64 * 0x1_0000 - i16::MIN as i64;
        ivec2(self.x as i32, y as i32)
    }

    /// Convert an unfolded 2D vector back to a Location.
    pub fn fold(loc_pos: impl Into<IVec2>) -> Self {
        let loc_pos = loc_pos.into();

        let x = loc_pos.x as i16;
        let y =
            ((loc_pos.y as i64).rem_euclid(0x1_0000) + i16::MIN as i64) as i16;
        let z = (loc_pos.y as i64).div_euclid(0x1_0000) as i16;

        Location { x, y, z }
    }

    /// Convenience method that doubles the x coordinate.
    ///
    /// Use for double-width character display.
    pub fn unfold_wide(&self) -> IVec2 {
        let mut ret = self.unfold();
        ret.x *= 2;
        ret
    }

    pub fn fold_wide(wide_loc_pos: impl Into<IVec2>) -> Option<Self> {
        let wide_loc_pos = wide_loc_pos.into();

        if wide_loc_pos.x % 2 == 0 {
            Some(Location::fold(wide_loc_pos / ivec2(2, 1)))
        } else {
            None
        }
    }

    pub fn smart_fold_wide(
        wide_loc_pos: impl Into<IVec2>,
        r: &impl AsRef<Runtime>,
    ) -> Self {
        match Self::fold_wide_sides(wide_loc_pos) {
            (a, b) if !a.is_explored(r) && b.is_explored(r) => b,
            (a, b)
                if a.entities_at(r).is_empty()
                    && !b.entities_at(r).is_empty() =>
            {
                b
            }
            (a, _) => a,
        }
    }

    /// Return the two locations on two sides of an off-center wide pos.
    ///
    /// If pos is not off-center, returns the same centered location twice.
    pub fn fold_wide_sides(wide_loc_pos: impl Into<IVec2>) -> (Self, Self) {
        let wide_loc_pos = wide_loc_pos.into();

        match Location::fold_wide(wide_loc_pos) {
            Some(loc) => (loc, loc),
            None => (
                Location::fold_wide(wide_loc_pos - ivec2(1, 0)).unwrap(),
                Location::fold_wide(wide_loc_pos + ivec2(1, 0)).unwrap(),
            ),
        }
    }

    pub fn to_vec3(&self) -> IVec3 {
        ivec3(self.x as i32, self.y as i32, self.z as i32)
    }

    pub fn tile(&self, r: &impl AsRef<Runtime>) -> Tile {
        let r = r.as_ref();
        r.terrain_overlay
            .get(self)
            .copied()
            .or_else(|| r.world.tile(self))
            .unwrap_or_default()
    }

    /// Get actual tiles from visible cells, assume ground for unexplored
    /// cell.
    pub fn assumed_tile(&self, r: &impl AsRef<Runtime>) -> Tile {
        if self.is_explored(r) {
            self.tile(r)
        } else {
            Tile::Ground
        }
    }

    pub fn set_tile(&self, r: &mut impl AsMut<Runtime>, t: Tile) {
        let r = r.as_mut();
        r.terrain_overlay.insert(*self, t);
    }

    /// Tile setter that doesn't cover functional terrain.
    pub fn decorate_tile(&self, r: &mut impl AsMut<Runtime>, t: Tile) {
        let r = r.as_mut();
        if self.tile(r) == Tile::Ground || self.tile(r).is_decoration() {
            r.terrain_overlay.insert(*self, t);
        }
    }

    /// Return location snapped to the origin of this location's sector.
    pub fn sector(&self) -> Location {
        Location::new(
            ((self.x as i32).div_floor(SECTOR_WIDTH) * SECTOR_WIDTH) as i16,
            ((self.y as i32).div_floor(SECTOR_HEIGHT) * SECTOR_HEIGHT) as i16,
            self.z,
        )
    }

    /// How many sector transitions there are between self and other.
    pub fn sector_dist(&self, other: &Location) -> usize {
        let a = Into::<IVec3>::into(self.sector())
            / ivec3(SECTOR_WIDTH, SECTOR_HEIGHT, 1);
        let b = Into::<IVec3>::into(other.sector())
            / ivec3(SECTOR_WIDTH, SECTOR_HEIGHT, 1);
        let d = (a - b).abs();
        (d.x + d.y + d.z) as usize
    }

    /// Return sector bounding box containing this loc.
    pub fn sector_bounds(&self) -> Rect {
        let p = self.sector().unfold();
        Rect::new(p, p + ivec2(SECTOR_WIDTH, SECTOR_HEIGHT))
    }

    pub fn at_sector_edge(&self) -> bool {
        let (u, v) = (
            self.x.rem_euclid(SECTOR_WIDTH as i16),
            self.y.rem_euclid(SECTOR_HEIGHT as i16),
        );
        (u == 0 || u == (SECTOR_WIDTH as i16 - 1))
            || (v == 0 || v == (SECTOR_HEIGHT as i16 - 1))
    }

    /// Return sector bounds extended for the adjacent sector rim.
    pub fn expanded_sector_bounds(&self) -> Rect {
        self.sector_bounds().grow([1, 1], [1, 1])
    }

    /// Location has been seen by an allied unit at some point.
    pub fn is_explored(&self, r: &impl AsRef<Runtime>) -> bool {
        let r = r.as_ref();
        r.fov.contains(self)
    }

    pub fn is_walkable(&self, r: &impl AsRef<Runtime>) -> bool {
        !self.tile(r).blocks_movement()
    }

    pub fn mob_at(&self, r: &impl AsRef<Runtime>) -> Option<Entity> {
        let r = r.as_ref();
        r.placement.entities_at(*self).find(|e| e.is_mob(r))
    }

    /// Return entities at cell sorted to draw order.
    pub fn entities_at(&self, r: &impl AsRef<Runtime>) -> Vec<Entity> {
        let r = r.as_ref();
        let mut ret: Vec<Entity> = r.placement.entities_at(*self).collect();
        ret.sort_by_key(|e| e.draw_layer(r));
        ret
    }

    pub fn item_at(&self, r: &impl AsRef<Runtime>) -> Option<Entity> {
        let r = r.as_ref();
        r.placement.entities_at(*self).find(|e| e.is_item(r))
    }

    pub fn vec_towards(&self, other: &Location) -> Option<IVec2> {
        if self.z == other.z {
            Some((*other - *self).truncate())
        } else {
            None
        }
    }

    pub fn vec3_towards(&self, other: &Location) -> IVec3 {
        Into::<IVec3>::into(*other) - Into::<IVec3>::into(*self)
    }

    /// Same sector plus the facing rims of adjacent sectors.
    pub fn has_same_screen_as(&self, other: &Location) -> bool {
        self.expanded_sector_bounds().contains(other.unfold())
    }

    /// Try to reconstruct step towards adjacent other location. Handles
    /// folding.
    pub fn find_step_towards(
        &self,
        r: &impl AsRef<Runtime>,
        other: &Location,
    ) -> Option<IVec2> {
        // They're on the same Z-plane, just do the normal pointing direction.
        if self.z == other.z {
            let a = self.unfold();
            let b = other.unfold();
            return Some(a.dir4_towards(&b));
        }

        // Otherwise look for immediate fold portals that lead to the other
        // loc.
        s4::DIR
            .into_iter()
            .find(|&d| (*self + d).follow(r) == *other)
    }

    /// Follow upstairs, downstairs and possible other portals until you end
    /// up at a non-portaling location starting from this location.
    pub fn follow(&self, r: &impl AsRef<Runtime>) -> Location {
        let path = || {
            let mut p = Some(*self);
            std::iter::from_fn(move || {
                let Some(loc) = p else {
                    return None;
                };
                let ret = p;
                p = loc.portal_dest(r);
                ret
            })
        };

        // If the map data is bad, there might be cycles, run a cycle
        // detection before trying to follow the path to the end.
        for (a, b) in path().zip(path().skip(1).step_by(2)) {
            if a == b {
                log::warn!(
                    "Location::fold: cycle detected starting from {self:?}"
                );
                return *self;
            }
        }

        path().last().unwrap_or(*self)
    }

    pub fn portal_dest(&self, r: &impl AsRef<Runtime>) -> Option<Location> {
        match self.tile(r) {
            Tile::Upstairs => Some(*self + ivec3(0, 0, 1)),
            Tile::Downstairs => Some(*self + ivec3(0, 0, -1)),
            _ => None,
        }
    }

    pub fn sector_locs(&self) -> impl Iterator<Item = Location> {
        let origin = self.sector();
        Rect::sized([SECTOR_WIDTH, SECTOR_HEIGHT])
            .into_iter()
            .map(move |p| origin + IVec2::from(p))
    }

    pub fn expanded_sector_locs(&self) -> impl Iterator<Item = Location> {
        let origin = self.sector();
        Rect::sized([SECTOR_WIDTH + 2, SECTOR_HEIGHT + 2])
            .into_iter()
            .map(move |p| origin + IVec2::from(p) - ivec2(1, 1))
    }

    /// Return the four neighbors to this location in an arbitrary order.
    pub fn perturbed_neighbors_4(&self) -> Vec<Location> {
        let mut rng = util::srng(self);
        let mut dirs: Vec<Location> =
            s4::DIR.iter().map(|&d| *self + d).collect();
        dirs.shuffle(&mut rng);
        dirs
    }

    pub fn neighbors_4(&self) -> impl Iterator<Item = Location> {
        // Alternate biasing based on location so algs will perform zig-zags
        // on diagonals.
        const H4: [IVec2; 4] = [
            IVec2::from_array([1, 0]),
            IVec2::from_array([-1, 0]),
            IVec2::from_array([0, 1]),
            IVec2::from_array([0, -1]),
        ];

        const V4: [IVec2; 4] = [
            IVec2::from_array([0, 1]),
            IVec2::from_array([0, -1]),
            IVec2::from_array([1, 0]),
            IVec2::from_array([-1, 0]),
        ];
        let o = *self;
        if ivec2(self.x as i32, self.y as i32).prefer_horizontals_here() {
            &H4
        } else {
            &V4
        }
        .iter()
        .map(move |d| o + *d)
    }

    pub fn fold_neighbors_4<'a>(
        &self,
        r: &'a Runtime,
    ) -> impl Iterator<Item = Location> + 'a {
        self.neighbors_4().map(move |loc| loc.follow(r))
    }

    pub fn astar_heuristic(&self, other: &Location) -> usize {
        // NB. This will work badly if pathing between Z-layers.
        let d = (*self - *other).abs();
        (d.x + d.y + d.z) as usize
    }

    /// Find the closest pathable location on neighboring sector.
    pub fn path_dest_to_neighboring_sector(
        &self,
        r: &impl AsRef<Runtime>,
        neighbor_dir: SectorDir,
    ) -> Option<Location> {
        for (loc, _) in util::dijkstra_map(
            move |loc| {
                let mut ret = Vec::new();
                for d in s4::DIR {
                    let loc = (*loc + d).follow(r);
                    if !loc.is_walkable(r) {
                        continue;
                    }

                    // Skip unexplored sectors, but allow one to get through
                    // if it gets us to destination (unmapped stairwell)
                    if loc.sector() != self.sector() + neighbor_dir
                        && !loc.is_explored(r)
                    {
                        continue;
                    }
                    let sd = loc.sector() - self.sector();
                    if sd != IVec3::ZERO && sd != neighbor_dir.to_vec3() {
                        continue;
                    }
                    ret.push(loc);
                }
                ret
            },
            vec![*self],
        ) {
            let sd = loc.sector() - self.sector();
            if sd == neighbor_dir.to_vec3() {
                return Some(loc);
            }
        }

        None
    }

    // Start tracing from self towards `dir` in `dir` size steps. Starts
    // from the point one step away from self. Panics if `dir` is a zero
    // vector. Does not follow portals.
    pub fn trace(&self, dir: IVec2) -> impl Iterator<Item = Location> {
        assert!(dir != IVec2::ZERO);

        let mut p = *self;
        std::iter::from_fn(move || {
            p += dir;
            Some(p)
        })
    }

    /// Create a printable description of interesting features at location.
    pub fn describe(&self, r: &impl AsRef<Runtime>) -> Option<String> {
        let mut ret = String::new();
        if let Some(mob) = self.mob_at(r) {
            ret.push_str(&Grammatize::format(&(mob.noun(r),), "[Some]"));
            if let Some(item) = self.item_at(r) {
                ret.push_str(&Grammatize::format(&(item.noun(r),), ", [some]"));
            }
            Some(ret)
        } else if let Some(item) = self.item_at(r) {
            ret.push_str(&Grammatize::format(&(item.noun(r),), "[Some]"));
            Some(ret)
        } else {
            None
        }
        // Add more stuff here as needed.
    }

    pub fn damage(
        &self,
        r: &mut impl AsMut<Runtime>,
        perp: Option<Entity>,
        amount: i32,
    ) {
        let r = r.as_mut();
        if let Some(mob) = self.mob_at(r) {
            mob.damage(r, perp, amount);
        }
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
