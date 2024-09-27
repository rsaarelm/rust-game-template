use content::{Cube, Zone};
use pathfinding::prelude::*;
use rand::seq::SliceRandom;
use util::{dijkstra_map, v3, Neighbors2D, Sdf};

use crate::{placement::Place, prelude::*};

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

    /// Parametrizable pathfinding.
    pub fn find_path_with<I>(
        &self,
        start: Location,
        neighbors: impl Fn(&Location) -> I,
        dest: &impl Sdf,
    ) -> Option<Vec<Location>>
    where
        I: IntoIterator<Item = Location>,
    {
        if let Some(mut path) = astar(
            &start,
            |a| neighbors(a).into_iter().map(|c| (c, 1)),
            |a| dest.sd(*a),
            |a| dest.sd(*a) <= 0,
        )
        .map(|(a, _)| a)
        {
            path.reverse();
            path.pop();
            Some(path)
        } else {
            None
        }
    }

    /// Pathfinding for player that approaches unexplored terrain
    /// optimistically.
    ///
    /// Paths within the fat slice of the current sector, can path to
    /// locations that are immediately outside the sector but reachable by a
    /// single step from the fat slice.
    pub fn fog_exploring_path(
        &self,
        origin: Location,
        current: Location,
        dest: &impl Sdf,
        is_exploring: bool,
    ) -> Option<Vec<Location>> {
        // Only explore in the local sector slice. Make it wide so optimistic
        // pathing to neighboring sectors works.
        let explore_area = origin.sector().fat().wide();

        // Follow known paths into nearby neighboring sectors too, except when
        // this is targeting unknown territory (is_exploring is true), in
        // which case stick to the local slice.
        let range = if is_exploring {
            explore_area
        } else {
            origin.sector().grow(
                [SECTOR_WIDTH, SECTOR_HEIGHT, 2],
                [SECTOR_WIDTH, SECTOR_HEIGHT, 2],
            )
        };

        self.find_path_with(
            current,
            |loc| {
                loc.fog_exploring_walk_neighbors(self, explore_area)
                    .filter(move |&loc| {
                        range.contains(loc) || dest.sd(loc) <= 0
                    })
                    .collect::<Vec<_>>()
            },
            dest,
        )
    }

    /// Pathfind for enemies that know all terrain.
    ///
    /// Works like `fog_exploring_path` except always paths along actually
    /// existing terrain.
    pub fn enemy_path(
        &self,
        start: Location,
        dest: &impl Sdf,
    ) -> Option<Vec<Location>> {
        let sec = start.sector().fat();
        self.find_path_with(
            start,
            move |loc| {
                loc.walk_neighbors(self)
                    .map(|(_, loc)| loc)
                    .filter(move |&loc| sec.contains(loc) || dest.sd(loc) <= 0)
                    .collect::<Vec<_>>()
            },
            dest,
        )
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
