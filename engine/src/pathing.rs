use content::{Cube, Zone, LEVEL_BASIS};
use pathfinding::prelude::*;
use rand::seq::SliceRandom;
use util::{dijkstra_map, v3, Neighbors2D, Sdf};

use crate::{placement::Place, prelude::*};

#[derive(Copy, Clone, Debug)]
pub enum FogPathing {
    /// Pathing has perfect terrain knowledge regardless of fog of war.
    ///
    /// Use for enemy AIs.
    Ignore,
    /// Pathing assumes it can pass through fog of war areas and will explore
    /// fogged areas in its path.
    Explore,
    /// Pathing treats areas under fog of war as impassable and sticks to
    /// known terrain. Will fail to find valid paths that are partially
    /// covered in fog.
    Avoid,
}

impl Runtime {
    pub fn autoexplore_map(
        &self,
        zone: &Cube,
        start: Location,
    ) -> HashMap<Location, usize> {
        let travel_zone = zone.fat();
        let ret: HashMap<Location, usize> = dijkstra_map(
            &|loc: &Location| {
                loc.walk_neighbors(self)
                    .map(|(_, x)| x)
                    .filter(|a| travel_zone.contains(*a))
            },
            zone.wide().into_iter().map(v3).filter(|loc| {
                if loc.is_impassable(self) {
                    return false;
                }

                let loc2 = loc.snap_above_floor(self);
                !loc2.is_explored(self)
                    || (loc2.is_explored(self)
                        && travel_zone.contains(*loc)
                        && loc.ns_8().any(|loc| {
                            !loc.snap_above_floor(self).is_explored(self)
                        }))
            }),
        )
        .collect();

        if !ret.contains_key(&start) {
            // Map must reach the starting location.
            Default::default()
        } else {
            ret
        }
    }

    /// Find a path from a starting point to a target volume.
    ///
    /// Intended for short-range pathfinding, not spanning multiple sectors.
    pub fn find_path(
        &self,
        fog_behavior: FogPathing,
        start: Location,
        // Destination volume.
        dest: &Cube,
    ) -> Option<Vec<Location>> {
        // NB. This cannot navigate between sectors that aren't directly
        // connected by moving off to the side. This is by design, if you need
        // non-trivial (ie. not just for the case where the target stepped
        // over the edge to the connected adjacent sector) multi-sector
        // navigation, you probably want some kind of secondary sector-level
        // pathing system.
        let domain =
            // Get the box containing both starting point and goal area.
            dest.grow_to_contain(start)
            // Expand it to the smallest enclosing box of level volumes.
            .intersecting_lattice(LEVEL_BASIS).to_cells(LEVEL_BASIS);

        let in_domain = |loc| domain.contains(loc) || dest.sd(loc) <= 0;

        let neighbors = |loc: &Location| {
            let mut ret = Vec::new();

            for (dir, a) in loc.walk_neighbors(self) {
                // Going out of search range and not in goal, abort.
                if !in_domain(a) {
                    continue;
                }

                if !a.is_explored(self) {
                    use FogPathing::*;
                    match fog_behavior {
                        Ignore => {}
                        Explore => {
                            for (loc, cost) in [
                                (loc + dir.extend(0), 1),
                                (loc + dir.extend(1), 1),
                                (loc + dir.extend(-1), 1),
                            ] {
                                if !loc.is_explored(self) && in_domain(loc) {
                                    ret.push((loc, cost));
                                }
                            }
                            continue;
                        }
                        Avoid => continue,
                    }
                }

                ret.push((a, 1));
            }

            ret
        };

        let (mut path, _) =
            astar(&start, neighbors, |&a| dest.sd(a), |&a| dest.sd(a) <= 0)?;

        path.reverse();
        path.pop();
        Some(path)
    }

    pub fn fill_positions(
        &self,
        start: Location,
    ) -> impl Iterator<Item = Location> + '_ {
        util::dijkstra_map(
            move |loc| {
                let loc = *loc;
                loc.walk_neighbors(self)
                    .map(|(_, loc2)| loc2)
                    .filter(move |loc2| loc.sector().fat().contains(*loc2))
            },
            [start],
        )
        .map(|n| n.0)
    }

    /// Start filling positions around given location while staying within
    /// the same sector and on walkable tiles.
    pub fn perturbed_fill_positions(
        &self,
        start: Location,
    ) -> impl Iterator<Item = Location> + '_ {
        util::dijkstra_map(
            move |&loc| {
                let mut elts = loc
                    .walk_neighbors(self)
                    .map(|(_, loc2)| loc2)
                    .filter(|loc2| loc.sector().fat().contains(*loc2))
                    .collect::<Vec<_>>();
                elts.shuffle(&mut util::srng(&loc));
                elts
            },
            [start],
        )
        .map(|n| n.0)
    }
}

impl Entity {
    /// Movement direction along a given Dijkstra map for given location, if
    /// the map provides any valid steps.
    pub fn dijkstra_map_direction(
        &self,
        r: &impl AsRef<Runtime>,
        map: &HashMap<Location, usize>,
        loc: Location,
    ) -> Option<IVec2> {
        let r = r.as_ref();

        // Default to max, always prefer stepping from non-map to map.
        let start = map.get(&loc).copied().unwrap_or(usize::MAX);

        if let Some((dir, n)) = loc
            .walk_neighbors(r)
            .filter_map(|(dir, loc)| {
                // Don't walk into enemies.
                if let Some(mob) = loc.mob_at(r) {
                    if self.is_enemy(r, &mob) {
                        return None;
                    }
                    // Friendlies are okay, assume they can be displaced.
                }
                map.get(&loc).map(|u| (dir, u))
            })
            .min_by_key(|(_, u)| *u)
        {
            // Allow neutral steps in case of something like
            //
            //     12
            //     22 <-
            //
            // Hope that a better gradient is found. This can cause endless
            // back-and-forth if caught in a pocket of flat cells with no way
            // out.
            if *n < start {
                return Some(dir);
            }
        }
        None
    }

    /// Given a starting place, find a nearby spot where the entity will fit
    /// comfortably. Returns the original pos if finding a different one
    /// fails.
    pub fn open_placement_spot(
        &self,
        r: &impl AsRef<Runtime>,
        place: impl Into<Place>,
    ) -> Place {
        let r = r.as_ref();
        match place.into() {
            // TODO: Take inventory limits into account, if the item doesn't
            // fit in recipient's inventory, recurse with new place at
            // recipient's location, so it'll spawn on your feet instead.
            Place::In(e) => Place::In(e),
            Place::At(loc) => Place::At(
                r.perturbed_fill_positions(loc)
                    .find(|&e| self.can_enter(r, e))
                    .unwrap_or(loc),
            ),
        }
    }

    /// Place an item near `loc`, deviating to avoid similar entities.
    ///
    /// Items will avoid other items, mobs will avoid other mobs.
    pub fn place_near(
        &self,
        r: &mut impl AsMut<Runtime>,
        place: impl Into<Place>,
    ) {
        let r = r.as_mut();
        self.place(r, self.open_placement_spot(r, place));
    }

    pub(crate) fn vec_towards(
        &self,
        r: &impl AsRef<Runtime>,
        other: &Entity,
    ) -> Option<IVec2> {
        let (Some(a), Some(b)) = (self.loc(r), other.loc(r)) else {
            return None;
        };
        a.vec2_towards(&b)
    }
}
