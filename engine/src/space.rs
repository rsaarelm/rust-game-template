use content::Rect;
use glam::{ivec2, ivec3, IVec2, IVec3};
use rand::prelude::*;
use util::s4;

use crate::{prelude::*, Grammatize, SectorDir};

pub trait RuntimeCoordinates: Copy + Sized {
    fn z(&self) -> i16;

    /// Convert to 2D vector, layering Z-levels vertically in 2-plane.
    ///
    /// Each location has a unique point on the `IVec2` plane and the original
    /// location can be retrieved by calling `Location::from` on the `IVec2`
    /// value.
    fn unfold(&self) -> IVec2;

    /// Convert an unfolded 2D vector back to a Location.
    fn fold(loc_pos: impl Into<IVec2>) -> Self;

    /// Convenience method that doubles the x coordinate.
    ///
    /// Use for double-width character display.
    fn unfold_wide(&self) -> IVec2 {
        let mut ret = self.unfold();
        ret.x *= 2;
        ret
    }

    fn fold_wide(wide_loc_pos: impl Into<IVec2>) -> Option<Self> {
        let wide_loc_pos = wide_loc_pos.into();

        if wide_loc_pos.x % 2 == 0 {
            Some(Self::fold(wide_loc_pos / ivec2(2, 1)))
        } else {
            None
        }
    }

    fn smart_fold_wide(
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
    fn fold_wide_sides(wide_loc_pos: impl Into<IVec2>) -> (Self, Self) {
        let wide_loc_pos = wide_loc_pos.into();

        match Self::fold_wide(wide_loc_pos) {
            Some(loc) => (loc, loc),
            None => (
                Self::fold_wide(wide_loc_pos - ivec2(1, 0)).unwrap(),
                Self::fold_wide(wide_loc_pos + ivec2(1, 0)).unwrap(),
            ),
        }
    }

    fn to_vec3(&self) -> IVec3;

    fn map_tile(&self, r: &impl AsRef<Runtime>) -> Tile2D;

    /// Get actual tiles from visible cells, assume ground for unexplored
    /// cell.
    fn assumed_tile(&self, r: &impl AsRef<Runtime>) -> Tile2D {
        if self.is_explored(r) {
            self.map_tile(r)
        } else {
            Tile2D::Ground
        }
    }

    fn set_tile(&self, r: &mut impl AsMut<Runtime>, t: Tile2D);

    /// Tile setter that doesn't cover functional terrain.
    fn decorate_tile(&self, r: &mut impl AsMut<Runtime>, t: Tile2D) {
        let r = r.as_mut();

        if self.map_tile(r) == Tile2D::Ground
            || self.map_tile(r).is_decoration()
        {
            self.set_tile(r, t);
        }
    }

    /// Return location snapped to the origin of this location's sector.
    fn sector(&self) -> Location;

    /// How many sector transitions there are between self and other.
    fn sector_dist(&self, other: &Self) -> usize;

    /// Return sector bounding box containing this loc.
    fn sector_bounds(&self) -> Rect {
        let p = self.sector().unfold();
        Rect::new(p, p + ivec2(SECTOR_WIDTH, SECTOR_HEIGHT))
    }

    fn at_sector_edge(&self) -> bool;

    /// Return sector bounds extended for the adjacent sector rim.
    fn expanded_sector_bounds(&self) -> Rect {
        self.sector_bounds().grow([1, 1], [1, 1])
    }

    /// Location has been seen by an allied unit at some point.
    fn is_explored(&self, r: &impl AsRef<Runtime>) -> bool;

    fn is_walkable(&self, r: &impl AsRef<Runtime>) -> bool {
        !self.map_tile(r).blocks_movement()
    }

    fn blocks_shot(&self, r: &impl AsRef<Runtime>) -> bool {
        match self.map_tile(r) {
            // Door is held open by someone passing through.
            Tile2D::Door if self.mob_at(r).is_some() => false,
            t => t.blocks_shot(),
        }
    }

    fn blocks_sight(&self, r: &impl AsRef<Runtime>) -> bool {
        match self.map_tile(r) {
            // Door is held open by someone passing through.
            Tile2D::Door if self.mob_at(r).is_some() => false,
            t => t.blocks_sight(),
        }
    }

    fn mob_at(&self, r: &impl AsRef<Runtime>) -> Option<Entity>;

    /// Return entities at cell sorted to draw order.
    fn entities_at(&self, r: &impl AsRef<Runtime>) -> Vec<Entity>;

    fn item_at(&self, r: &impl AsRef<Runtime>) -> Option<Entity>;

    //////
    fn vec_towards(&self, other: &Location) -> Option<IVec2>;

    fn vec3_towards(&self, other: &Location) -> IVec3;

    /// Same sector plus the facing rims of adjacent sectors.
    fn has_same_screen_as(&self, other: &Self) -> bool {
        self.expanded_sector_bounds().contains(other.unfold())
    }

    /// Try to reconstruct step towards adjacent other location. Handles
    /// folding.
    fn find_step_towards(
        &self,
        r: &impl AsRef<Runtime>,
        other: &Self,
    ) -> Option<IVec2>;

    /// Follow upstairs, downstairs and possible other portals until you end
    /// up at a non-portaling location starting from this location.
    fn follow(&self, r: &impl AsRef<Runtime>) -> Self;

    fn portal_dest(&self, r: &impl AsRef<Runtime>) -> Option<Self>;

    fn sector_locs(&self) -> impl Iterator<Item = Self>;

    fn expanded_sector_locs(&self) -> impl Iterator<Item = Self>;

    /// Return the four neighbors to this location in an arbitrary order.
    fn perturbed_flat_neighbors_4(&self) -> Vec<Self>;

    fn flat_neighbors_4(&self) -> impl Iterator<Item = Self> + '_;

    fn fold_neighbors_4(&self, r: &Runtime) -> Vec<Self> {
        // TODO Figure out lifetime annotations to turn return value into iterator
        self.flat_neighbors_4()
            .map(move |loc| loc.follow(r))
            .collect()
    }

    fn astar_heuristic(&self, other: &Self) -> usize;

    /// Find the closest pathable location on neighboring sector.
    fn path_dest_to_neighboring_sector(
        &self,
        r: &impl AsRef<Runtime>,
        neighbor_dir: SectorDir,
    ) -> Option<Self>;

    // Start tracing from self towards `dir` in `dir` size steps. Starts
    // from the point one step away from self. Panics if `dir` is a zero
    // vector. Does not follow portals.
    fn trace(&self, dir: IVec2) -> impl Iterator<Item = Self>;

    /// Create a printable description of interesting features at location.
    fn describe(&self, r: &impl AsRef<Runtime>) -> Option<String> {
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

    /// Description for the general area of the location.
    fn region_name(&self, _r: &impl AsRef<Runtime>) -> String {
        let depth = -self.z();
        format!("Mazes of Menace: {depth}")
    }

    fn damage(
        &self,
        r: &mut impl AsMut<Runtime>,
        perp: Option<Entity>,
        amount: i32,
    );
}

impl RuntimeCoordinates for Location {
    fn z(&self) -> i16 {
        self.z
    }

    fn unfold(&self) -> IVec2 {
        // Maps y: i16::MIN, z: i16::MIN to i32::MIN.
        let y = self.y as i64 + self.z as i64 * 0x1_0000 - i16::MIN as i64;
        ivec2(self.x as i32, y as i32)
    }

    fn fold(loc_pos: impl Into<IVec2>) -> Self {
        let loc_pos = loc_pos.into();

        let x = loc_pos.x as i16;
        let y =
            ((loc_pos.y as i64).rem_euclid(0x1_0000) + i16::MIN as i64) as i16;
        let z = (loc_pos.y as i64).div_euclid(0x1_0000) as i16;

        Location::new(x, y, z)
    }

    fn to_vec3(&self) -> IVec3 {
        ivec3(self.x as i32, self.y as i32, self.z as i32)
    }

    fn map_tile(&self, r: &impl AsRef<Runtime>) -> Tile2D {
        let r = r.as_ref();
        r.world.get(&(*self).into())
    }

    fn set_tile(&self, r: &mut impl AsMut<Runtime>, t: Tile2D) {
        let r = r.as_mut();
        r.world.set(&(*self).into(), t);
    }

    fn sector(&self) -> Location {
        Location::new(
            ((self.x as i32).div_floor(SECTOR_WIDTH) * SECTOR_WIDTH) as i16,
            ((self.y as i32).div_floor(SECTOR_HEIGHT) * SECTOR_HEIGHT) as i16,
            self.z,
        )
    }

    fn sector_dist(&self, other: &Self) -> usize {
        let a = Into::<IVec3>::into(self.sector())
            / ivec3(SECTOR_WIDTH, SECTOR_HEIGHT, 1);
        let b = Into::<IVec3>::into(other.sector())
            / ivec3(SECTOR_WIDTH, SECTOR_HEIGHT, 1);
        let d = (a - b).abs();
        (d.x + d.y + d.z) as usize
    }

    fn at_sector_edge(&self) -> bool {
        let (u, v) = (
            self.x.rem_euclid(SECTOR_WIDTH as i16),
            self.y.rem_euclid(SECTOR_HEIGHT as i16),
        );
        (u == 0 || u == (SECTOR_WIDTH as i16 - 1))
            || (v == 0 || v == (SECTOR_HEIGHT as i16 - 1))
    }

    /// Return sector bounds extended for the adjacent sector rim.
    fn expanded_sector_bounds(&self) -> Rect {
        self.sector_bounds().grow([1, 1], [1, 1])
    }

    fn is_explored(&self, r: &impl AsRef<Runtime>) -> bool {
        let r = r.as_ref();
        r.fov.contains(self)
    }

    fn mob_at(&self, r: &impl AsRef<Runtime>) -> Option<Entity> {
        let r = r.as_ref();
        r.placement.entities_at(self).find(|e| e.is_mob(r))
    }

    fn entities_at(&self, r: &impl AsRef<Runtime>) -> Vec<Entity> {
        let r = r.as_ref();
        let mut ret: Vec<Entity> = r.placement.entities_at(self).collect();
        ret.sort_by_key(|e| e.draw_layer(r));
        ret
    }

    fn item_at(&self, r: &impl AsRef<Runtime>) -> Option<Entity> {
        let r = r.as_ref();
        r.placement.entities_at(self).find(|e| e.is_item(r))
    }

    fn vec_towards(&self, other: &Location) -> Option<IVec2> {
        if self.z == other.z {
            Some((*other - *self).truncate())
        } else {
            None
        }
    }

    fn vec3_towards(&self, other: &Location) -> IVec3 {
        Into::<IVec3>::into(*other) - Into::<IVec3>::into(*self)
    }

    fn find_step_towards(
        &self,
        r: &impl AsRef<Runtime>,
        other: &Self,
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

    fn follow(&self, r: &impl AsRef<Runtime>) -> Self {
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

    fn portal_dest(&self, r: &impl AsRef<Runtime>) -> Option<Self> {
        match self.map_tile(r) {
            Tile2D::Upstairs => Some(*self + ivec3(0, 0, 1)),
            Tile2D::Downstairs => Some(*self + ivec3(0, 0, -1)),
            _ => None,
        }
    }

    fn sector_locs(&self) -> impl Iterator<Item = Self> {
        let origin = self.sector();
        Rect::sized([SECTOR_WIDTH, SECTOR_HEIGHT])
            .into_iter()
            .map(move |p| origin + IVec2::from(p))
    }

    fn expanded_sector_locs(&self) -> impl Iterator<Item = Self> {
        let origin = self.sector();
        Rect::sized([SECTOR_WIDTH + 2, SECTOR_HEIGHT + 2])
            .into_iter()
            .map(move |p| origin + IVec2::from(p) - ivec2(1, 1))
    }

    fn perturbed_flat_neighbors_4(&self) -> Vec<Self> {
        let mut rng = util::srng(self);
        let mut dirs: Vec<Location> =
            s4::DIR.iter().map(|&d| *self + d).collect();
        dirs.shuffle(&mut rng);
        dirs
    }

    fn flat_neighbors_4(&self) -> impl Iterator<Item = Self> {
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

    fn astar_heuristic(&self, other: &Self) -> usize {
        // NB. This will work badly if pathing between Z-layers.
        let d = (*self - *other).abs();
        (d.x + d.y + d.z) as usize
    }

    fn path_dest_to_neighboring_sector(
        &self,
        r: &impl AsRef<Runtime>,
        neighbor_dir: SectorDir,
    ) -> Option<Self> {
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

    fn trace(&self, dir: IVec2) -> impl Iterator<Item = Self> {
        assert!(dir != IVec2::ZERO);

        let mut p = *self;
        std::iter::from_fn(move || {
            p += dir;
            Some(p)
        })
    }

    fn damage(
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