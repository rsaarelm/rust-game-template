use rand::prelude::*;
use util::{s4, RngExt};

use crate::{
    prelude::*, Cube, Data, EntitySeed, FlatPatch, Patch, Rect, Spawn,
};

pub trait MapGenerator {
    fn run(&self, rng: &mut dyn RngCore, lot: &Lot) -> anyhow::Result<Patch>;
}

/// Bounds and topology definition for map generation.
#[derive(Copy, Clone, Debug, Default)]
pub struct Lot {
    /// Volume in space in which the map should be generated.
    _volume: Cube,

    /// Connection flags to the six neighbors. If the bit is set for a given
    /// edge, the map generator is expected to generate a connection in that
    /// direction. The bit order is NESWDU.
    _connections: u8,
}

#[derive(Copy, Clone, Default)]
pub struct Level {
    /// Dungeon depth, greater depth has more powerful monsters and items.
    depth: u32,
    /// Upstairs position to generate into the map.
    upstairs: Option<IVec2>,
    /// True if map should include a downstairs exit.
    generate_downstairs: bool,
}

impl Level {
    pub fn new(depth: u32) -> Self {
        Level {
            depth,
            ..Default::default()
        }
    }

    pub fn upstairs_at(mut self, pos: IVec2) -> Self {
        self.upstairs = Some(pos);
        self
    }

    pub fn with_downstairs(mut self) -> Self {
        self.generate_downstairs = true;
        self
    }

    /// Generate a random creature.
    fn creature(&self, rng: &mut (impl Rng + ?Sized)) -> Spawn {
        // TODO Cache the distribution. (Good place for a memoizing func?)
        let spawns: Vec<_> = Data::get()
            .bestiary
            .iter()
            .filter(|(_, m)| m.min_depth() <= self.depth)
            .collect();

        let (n, _) = spawns
            .choose_weighted(rng, |(_, m)| m.spawn_weight())
            .unwrap();
        (*n).into()
    }

    /// Generate a random item.
    fn item(&self, rng: &mut (impl Rng + ?Sized)) -> Spawn {
        let spawns: Vec<_> = Data::get()
            .armory
            .iter()
            .filter(|(_, m)| m.min_depth() <= self.depth)
            .collect();

        let (n, _) = spawns
            .choose_weighted(rng, |(_, m)| m.spawn_weight())
            .unwrap();
        (*n).into()
    }

    /// Generate a random rectangular room.
    fn room(&self, rng: &mut (impl Rng + ?Sized)) -> FlatPatch {
        let mut ret = FlatPatch::default();

        let w = rng.gen_range(2..=10);
        let h = rng.gen_range(2..=10);

        // Set corners as undiggable.
        ret.set_terrain([-1, -1], MapTile::Wall);
        ret.set_terrain([w, -1], MapTile::Wall);
        ret.set_terrain([-1, h], MapTile::Wall);
        ret.set_terrain([w, h], MapTile::Wall);

        ret.entrance = Some(ivec2(rng.gen_range(0..w), rng.gen_range(0..h)));

        for p in Rect::sized([w, h]) {
            ret.set_terrain(p, MapTile::Ground);
            if rng.one_chance_in(60) && Some(p.into()) != ret.entrance {
                if rng.one_chance_in(3) {
                    ret.add_spawn(p, self.item(rng));
                } else {
                    ret.add_spawn(p, self.creature(rng));
                }
            }
        }

        ret
    }
}

impl Distribution<FlatPatch> for Level {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> FlatPatch {
        let level_area = Rect::sized([SECTOR_WIDTH, SECTOR_HEIGHT]);
        let mut ret = FlatPatch::default();

        if let Some(p) = self.upstairs {
            ret.set_terrain(p, MapTile::Upstairs);
            ret.set_terrain(p + ivec2(0, 1), MapTile::Ground);

            // Undiggable enclosure.
            ret.set_terrain(p + ivec2(-1, 0), MapTile::Wall);
            ret.set_terrain(p + ivec2(1, 0), MapTile::Wall);
            ret.set_terrain(p + ivec2(-1, -1), MapTile::Wall);
            ret.set_terrain(p + ivec2(0, -1), MapTile::Wall);
            ret.set_terrain(p + ivec2(1, -1), MapTile::Wall);
        }

        'placement: for _ in 0..rng.gen_range(6..18) {
            let new_room = self.room(rng);
            let room_bounds = new_room.bounds();
            let mut posns: Vec<IVec2> = level_area
                .into_iter()
                .filter(|&p| level_area.contains_other(&(room_bounds + p)))
                .map(|c| c.into())
                .collect();
            posns.shuffle(rng);

            while let Some(p) = posns.pop() {
                if !ret.can_place(p, &new_room) {
                    continue;
                }

                let mut map = ret.clone();
                // Open positions in original (if any)
                let start_cells: Vec<_> = map.open_area().collect();
                // Open positions in result.
                let end_cells: Vec<_> =
                    new_room.open_area().map(|a| a + p).collect();
                map.merge(p, new_room.clone());

                // There's no floor when we start out, assume this is the case
                // here and just place the room and continue.
                let Some(&start) = start_cells.choose(rng) else {
                    ret = map;
                    continue;
                };

                let &end = end_cells.choose(rng).expect("no floor in room");

                // Try to path from the new room back to existing map. Hitting
                // any open space in the existing map ends the tunneling.

                let Some((path, _)) = pathfinding::prelude::astar(
                    &start,
                    |p| map.valid_tunnels_from(p).map(|c| (c, 1)),
                    |p| s4::d(p, &end),
                    |p| end_cells.contains(p),
                ) else {
                    continue;
                };

                let mut prev = None;
                for p in path {
                    if !map.terrain.get(&p).map_or(false, |t| t.is_walkable()) {
                        if prev.is_none() {
                            map.set_terrain(p, MapTile::Door);
                        } else {
                            map.set_terrain(p, MapTile::Ground);
                        }
                        prev = Some(p);
                    }
                }

                if let Some(prev) = prev {
                    map.set_terrain(prev, MapTile::Door);
                }

                ret = map;
                continue 'placement;
            }

            // Found no places for new room if we fell down here.
            break;
        }

        if self.generate_downstairs {
            // Don't put it too close to the edge so the opposite stairs don't
            // end up being placed weirdly.
            let posns: Vec<_> = ret
                .downstair_positions()
                .filter(|p| p.y > 8 && p.y < SECTOR_HEIGHT - 8)
                .collect();

            // XXX: Hard panic inside level generator is bad, this should be a
            // fallible function.
            let p = *posns.choose(rng).expect("couldn't place downstairs");

            ret.set_terrain(p, MapTile::Downstairs);
            ret.set_terrain(p + ivec2(0, -1), MapTile::Ground);

            // Undiggable enclosure.
            ret.set_terrain(p + ivec2(-1, 0), MapTile::Wall);
            ret.set_terrain(p + ivec2(1, 0), MapTile::Wall);
            ret.set_terrain(p + ivec2(-1, 1), MapTile::Wall);
            ret.set_terrain(p + ivec2(0, 1), MapTile::Wall);
            ret.set_terrain(p + ivec2(1, 1), MapTile::Wall);
        }

        ret
    }
}
