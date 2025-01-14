use glam::ivec3;
use util::{s4, s8, Neighbors2D};
use world::{Block, Environs, Tile};

use crate::{prelude::*, Grammatize};

pub trait RuntimeCoordinates: Coordinates {
    /// Tile setter that doesn't cover functional terrain.
    fn decorate_block(&self, r: &mut impl AsMut<Runtime>, b: Block);

    /// Internal method for FoV
    fn is_in_fov_set(&self, r: &impl AsRef<Runtime>) -> bool;

    /// Location has been seen by an allied unit at some point.
    fn is_explored(&self, r: &impl AsRef<Runtime>) -> bool;

    /// Destination for UI path selection, may dip outside the +/-1 slice if
    /// the point is a wall above/below a position reached from an adjacent
    /// slope.
    fn ui_path_destination(&self, r: &impl AsRef<Runtime>) -> Self;

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

    /// Location has interactable terrain that can be bumped into, like an
    /// altar.
    fn is_interactable(&self, r: &impl AsRef<Runtime>) -> bool {
        let r = r.as_ref();
        self.voxel(r) == Some(Block::Altar)
    }

    /// Create a printable description of interesting objects at location.
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

    /// Return printable ambient description when player is standing at
    /// location.
    fn ambient_description(
        &self,
        r: &impl AsRef<Runtime>,
    ) -> Option<&'static str> {
        let r = r.as_ref();

        // Look for altar.
        for d in s4::DIR {
            let loc_2 = *self + d.extend(0);
            if loc_2.voxel(r) == Some(Block::Altar) {
                return Some("An ancient stone altar stands here.");
            }
        }

        None
    }

    /// Description for the general area of the location.
    fn region_name(&self, _r: &impl AsRef<Runtime>) -> String {
        let depth = -self.z().div_euclid(2);
        if depth > 0 {
            format!("Mazes of Menace: {depth}")
        } else {
            "Surface world".into()
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
            Some(Stone) | Some(SplatteredStone) | Some(Grass)
        ) {
            r.set_voxel(*self, Some(b));
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

    fn ui_path_destination(&self, r: &impl AsRef<Runtime>) -> Self {
        let r = r.as_ref();

        // The starting point must be explored space.
        if !self.is_explored(r) {
            return *self;
        }

        // Regular floors are returned as is.
        if let Tile::Surface(loc, _) = self.tile(r) {
            return loc;
        }

        // Now look for neighboring slopes that lead to a walkable tile just
        // off the slice.
        let mut candidates = HashSet::default();
        for d in s4::DIR {
            let Tile::Surface(loc_2, _) = (*self + d.extend(0)).tile(r) else {
                continue;
            };
            if let Some(loc) = loc_2.walk_step(r, -d) {
                candidates.insert(loc);
            }
        }

        // This only works if there's exactly one candidate cell, it's
        // possible for one path to lead to a location above and another to a
        // location below, in which case we won't try to choose.
        if candidates.len() == 1 {
            return candidates.into_iter().next().unwrap();
        }

        *self
    }

    fn entities_at<'a>(
        &self,
        r: &'a impl AsRef<Runtime>,
    ) -> impl Iterator<Item = Entity> + 'a {
        let r = r.as_ref();
        r.placement.entities_at(*self)
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
