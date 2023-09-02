use rand::prelude::*;
use util::{astar_path, s4, RngExt};

use crate::{prelude::*, Data, Germ, Patch, Rect, Spawn};

#[derive(Copy, Clone, Default)]
pub struct Level {
    /// Dungeon depth, greater depth has more powerful monsters and items.
    depth: u32,
    /// Upstairs position to generate into the map.
    _upstairs: Option<IVec2>,
    /// True if map should include a downstairs exit.
    _generate_downstairs: bool,
}

impl Level {
    pub fn new(depth: u32) -> Self {
        Level {
            depth,
            ..Default::default()
        }
    }

    pub fn _upstairs_at(mut self, pos: IVec2) -> Self {
        self._upstairs = Some(pos);
        self
    }

    pub fn _with_downstairs(mut self) -> Self {
        self._generate_downstairs = true;
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
    fn room(&self, rng: &mut (impl Rng + ?Sized)) -> Patch {
        let mut ret = Patch::default();

        let w = rng.gen_range(2..=10);
        let h = rng.gen_range(2..=10);

        // Set corners as undiggable.
        ret.set_terrain([-1, -1], Tile::Wall);
        ret.set_terrain([w, -1], Tile::Wall);
        ret.set_terrain([-1, h], Tile::Wall);
        ret.set_terrain([w, h], Tile::Wall);

        ret.entrance = Some(ivec2(rng.gen_range(0..w), rng.gen_range(0..h)));

        for p in Rect::sized([w, h]) {
            ret.set_terrain(p, Tile::Ground);
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

impl Distribution<Patch> for Level {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Patch {
        let level_area = Rect::sized([SECTOR_WIDTH, SECTOR_HEIGHT]);
        let mut ret = Patch::default();

        'placement: for _ in 0..rng.gen_range(6..18) {
            let mut new_room = self.room(rng);
            let room_bounds = new_room.bounds();
            let mut posns: Vec<IVec2> = level_area
                .into_iter()
                .filter(|&p| level_area.contains_other(&(room_bounds + p)))
                .map(|c| c.into())
                .collect();
            posns.shuffle(rng);

            while let Some(p) = posns.pop() {
                if ret.can_place(p, &new_room) {
                    let start_cells: Vec<_> = ret
                        .open_area()
                        .filter(|&a| !ret.is_tunnel(a))
                        .map(|a| a - p)
                        .collect();
                    let end_cells: Vec<_> = new_room.open_area().collect();
                    if let Some(&start) = start_cells.choose(rng) {
                        // Try to find a tunnel between two random points.
                        let &end =
                            end_cells.choose(rng).expect("No floor in room");

                        let Some(path) = astar_path(
                            &start,
                            &end,
                            |&a| {
                                let ret = &ret;
                                let new_room = &new_room;
                                s4::ns(a).filter(|&a| {
                                    new_room.can_tunnel(a)
                                        && ret.can_tunnel(a - p)
                                        && (level_area - p).contains(a)
                                })
                            },
                            s4::d,
                        ) else {
                            break 'placement;
                        };

                        for p in path {
                            if !new_room.terrain.contains_key(&p) {
                                new_room.set_terrain(p, Tile::Ground);
                            }
                        }
                    }

                    ret.merge(p, new_room);
                    continue 'placement;
                }
            }

            // Found no places for new room if we fell down here.
            break;
        }

        ret
    }
}
