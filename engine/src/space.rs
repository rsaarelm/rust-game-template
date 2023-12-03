use content::{Coordinates, Rect};
use glam::{ivec2, ivec3, IVec2, IVec3};
use rand::prelude::*;
use util::{s4, v3};

use crate::{prelude::*, Grammatize, SectorDir};

pub trait RuntimeCoordinates: Coordinates {
    fn smart_fold_wide(
        wide_loc_pos: impl Into<IVec2>,
        r: &impl AsRef<Runtime>,
    ) -> Self {
        match Self::fold_wide_sides(wide_loc_pos) {
            (a, b) if !a.is_explored(r) && b.is_explored(r) => b,
            (a, b)
                if a.entities_at(r).next().is_none()
                    && !b.entities_at(r).next().is_none() =>
            {
                b
            }
            (a, _) => a,
        }
    }

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

    fn entities_at<'a>(
        &self,
        r: &'a impl AsRef<Runtime>,
    ) -> impl Iterator<Item = Entity> + 'a;

    /// Return entities at cell sorted to draw order.
    fn drawable_entities_at(&self, r: &impl AsRef<Runtime>) -> Vec<Entity> {
        let mut ret: Vec<Entity> = self.entities_at(r).collect();
        ret.sort_by_key(|e| e.draw_layer(r));
        ret
    }

    fn mob_at(&self, r: &impl AsRef<Runtime>) -> Option<Entity> {
        self.entities_at(r).find(|e| e.is_mob(r))
    }

    fn item_at(&self, r: &impl AsRef<Runtime>) -> Option<Entity> {
        self.entities_at(r).find(|e| e.is_item(r))
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

    /// Find the closest pathable location on neighboring sector.
    fn path_dest_to_neighboring_sector(
        &self,
        r: &impl AsRef<Runtime>,
        neighbor_dir: SectorDir,
    ) -> Option<Self>;

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
    fn map_tile(&self, r: &impl AsRef<Runtime>) -> Tile2D {
        let r = r.as_ref();
        r.world.get(self)
    }

    fn set_tile(&self, r: &mut impl AsMut<Runtime>, t: Tile2D) {
        let r = r.as_mut();
        r.world.set(self, t);
    }

    fn is_explored(&self, r: &impl AsRef<Runtime>) -> bool {
        let r = r.as_ref();
        r.fov.contains(self)
    }

    fn entities_at<'a>(
        &self,
        r: &'a impl AsRef<Runtime>,
    ) -> impl Iterator<Item = Entity> + 'a {
        let r = r.as_ref();
        r.placement.entities_at(self)
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
            .find(|&d| (*self + d.extend(0)).follow(r) == *other)
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
            .map(move |p| origin + IVec2::from(p).extend(0))
    }

    fn expanded_sector_locs(&self) -> impl Iterator<Item = Self> {
        let origin = self.sector();
        Rect::sized([SECTOR_WIDTH + 2, SECTOR_HEIGHT + 2])
            .into_iter()
            .map(move |p| origin + (IVec2::from(p) - ivec2(1, 1)).extend(0))
    }

    fn perturbed_flat_neighbors_4(&self) -> Vec<Self> {
        let mut rng = util::srng(self);
        let mut dirs: Vec<Location> =
            s4::DIR.iter().map(|&d| *self + d.extend(0)).collect();
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
        .map(move |d| o + d.extend(0))
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
                    let loc = (*loc + d.extend(0)).follow(r);
                    if !loc.is_walkable(r) {
                        continue;
                    }

                    // Skip unexplored sectors, but allow one to get through
                    // if it gets us to destination (unmapped stairwell)
                    if loc.sector() != self.sector() + v3(neighbor_dir)
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
