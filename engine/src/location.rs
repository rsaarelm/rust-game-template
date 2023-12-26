use content::{Block, Coordinates, Environs};
use glam::ivec3;
use util::{s4, s8, Neighbors2D};

use crate::{prelude::*, Grammatize};

pub trait RuntimeCoordinates: Coordinates {
    /// Tile setter that doesn't cover functional terrain.
    fn decorate_block(&self, r: &mut impl AsMut<Runtime>, b: Block);

    /// Internal method for FoV
    fn is_in_fov_set(&self, r: &impl AsRef<Runtime>) -> bool;

    /// Location has been seen by an allied unit at some point.
    fn is_explored(&self, r: &impl AsRef<Runtime>) -> bool;

    /// List steppable neighbors optimistically, any unexplored neighbor cell
    /// is listed as rising, falling and horizontal steps. Explored neighbors
    /// will only provide the actual step, if any.
    fn fog_exploring_walk_neighbors<'a>(
        &self,
        r: &'a impl AsRef<Runtime>,
    ) -> impl Iterator<Item = Self> + 'a;

    fn blocks_shot(&self, r: &impl AsRef<Runtime>) -> bool {
        let r = r.as_ref();
        match self.voxel(r) {
            // Door is held open by someone passing through.
            Some(Block::Door) if self.mob_at(r).is_some() => false,
            Some(_) => true,
            None => false,
        }
    }

    fn blocks_sight(&self, r: &impl AsRef<Runtime>) -> bool {
        let r = r.as_ref();
        match self.voxel(r) {
            // Door is held open by someone passing through.
            Some(Block::Door) if self.mob_at(r).is_some() => false,
            Some(b) => b.blocks_sight(),
            None => false,
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
        let depth = -self.z().div_floor(2);
        if depth > 0 {
            format!("Mazes of Menace: {depth}")
        } else {
            format!("Surface world")
        }
    }

    fn damage(
        &self,
        r: &mut impl AsMut<Runtime>,
        perp: Option<Entity>,
        amount: i32,
    );
}

impl RuntimeCoordinates for Location {
    fn decorate_block(&self, r: &mut impl AsMut<Runtime>, b: Block) {
        let r = r.as_mut();

        use Block::*;

        if matches!(
            self.voxel(r),
            Some(Rock) | Some(SplatteredRock) | Some(Grass)
        ) {
            r.set_voxel(self, Some(b));
        }
    }

    fn is_in_fov_set(&self, r: &impl AsRef<Runtime>) -> bool {
        let r = r.as_ref();

        r.fov.contains(self)
    }

    fn is_explored(&self, r: &impl AsRef<Runtime>) -> bool {
        let r = r.as_ref();
        if self.snap_above_floor(r).is_in_fov_set(r) {
            return true;
        }

        if self.tile(r).is_wall() {
            // Any 4-adjacent visible cell makes a wall visible.
            if self
                .ns_4()
                .any(|loc| loc.snap_above_floor(r).is_in_fov_set(r))
            {
                return true;
            }

            for diag in s8::DIAGONALS.iter().map(|&p| p.extend(0)) {
                // This is a corner wall and next to a visible floor.
                //
                // Since last step didn't return, there's no directly adjacent
                // visible floor.
                //
                // Must have two adjacent walls to qualify as visible here.
                if (*self + diag).snap_above_floor(r).is_in_fov_set(r) {
                    if (*self + diag * ivec3(1, 0, 0)).tile(r).is_wall()
                        && (*self + diag * ivec3(0, 1, 0)).tile(r).is_wall()
                    {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn fog_exploring_walk_neighbors<'a>(
        &self,
        r: &'a impl AsRef<Runtime>,
    ) -> impl Iterator<Item = Self> + 'a {
        let r = r.as_ref();
        let origin = *self;

        s4::DIR.iter().flat_map(move |&dir| {
            let loc = origin + dir.extend(0);
            if loc.is_explored(r) {
                // Only step into the concrete location (if any) when target
                // is explored.
                origin.walk_step(r, dir).into_iter().collect::<Vec<_>>()
            } else {
                // Assume all possible steps are valid in unexplored space.
                vec![loc, loc.above(), loc.below()]
            }
        })
    }

    fn entities_at<'a>(
        &self,
        r: &'a impl AsRef<Runtime>,
    ) -> impl Iterator<Item = Entity> + 'a {
        let r = r.as_ref();
        r.placement.entities_at(self)
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
