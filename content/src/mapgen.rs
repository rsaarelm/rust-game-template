use anyhow::bail;
use derive_more::{Deref, DerefMut};
use glam::{ivec3, IVec2, IVec3};
use rand::{distributions::Distribution, seq::SliceRandom, Rng, RngCore};
use util::{
    a3, v3, Cloud, HashMap, HashSet, IndexMap, IndexSet, Logos, Neighbors2D,
};

use crate::{
    data::GenericSector, world, Block, Coordinates, Cube, Data, Environs,
    Level, Location, SectorMap, Spawn, SpawnDist, Voxel, Zone, SECTOR_HEIGHT,
    SECTOR_WIDTH,
};

pub trait MapGenerator {
    fn run(&self, rng: &mut dyn RngCore, lot: &Lot) -> anyhow::Result<Patch>;
}

impl<F> MapGenerator for F
where
    F: Fn(&mut dyn RngCore, &Lot) -> anyhow::Result<Patch>,
{
    fn run(&self, rng: &mut dyn RngCore, lot: &Lot) -> anyhow::Result<Patch> {
        self(rng, lot)
    }
}

impl MapGenerator for Patch {
    fn run(&self, _rng: &mut dyn RngCore, _lot: &Lot) -> anyhow::Result<Patch> {
        Ok(self.clone())
    }
}

impl MapGenerator for GenericSector {
    fn run(&self, rng: &mut dyn RngCore, lot: &Lot) -> anyhow::Result<Patch> {
        use GenericSector::*;

        match self {
            Water => todo!(),
            Grassland => todo!(),
            Forest => todo!(),
            Mountains => todo!(),
            // TODO: Proper dungeon generator.
            Dungeon => bigroom(rng, lot),
        }
    }
}

/// Bounds and topology definition for map generation.
#[derive(Copy, Clone, Debug)]
pub struct Lot {
    /// Volume in space in which the map should be generated.
    pub volume: Cube,

    /// Connection flags to the four horizontal neighbors. The bit order is
    /// NESW.
    ///
    /// If a connection bit is set, the generated map is expected to connect
    /// to the single center (rounded down) cell in its corresponding edge. Ie
    /// if there is a north connection (`lot.sides & 0b1 != 0`), the generated
    /// map must have a path to (2 * ⌊`SECTOR_WIDTH` / 4⌋, 0) (snap to even
    /// coordinates).
    pub sides: u8,

    pub up: Option<Location>,
    pub down: Option<Location>,
}

// Currently only using EAST and SOUTH as visible edge to other sectors is on
// the outer rectangle edge

//const NORTH: u8 = 0b1;
const EAST: u8 = 0b10;
const SOUTH: u8 = 0b100;
//const WEST: u8 = 0b1000;

impl Default for Lot {
    fn default() -> Self {
        let volume = Level::level_from(&Default::default());
        let sides = 0;
        let seed = Logos::default();
        let up = Some(world::default_down_stairs(&seed, volume.above()));
        let down = Some(world::default_down_stairs(&seed, volume));

        Lot {
            volume,
            sides,
            up,
            down,
        }
    }
}

impl Lot {
    pub fn new(
        volume: Cube,
        sides: u8,
        up: Option<Location>,
        down: Option<Location>,
    ) -> anyhow::Result<Self> {
        use crate::world::snap_stairwell_position;

        // Validate that stair positions fit in pattern.
        if let Some(up) = up {
            let expected = snap_stairwell_position(up);
            if up != expected {
                bail!("Bad upstairs spot in Lot: {up}, closest match is {expected}");
            }
        }
        if let Some(down) = down {
            let expected = snap_stairwell_position(down);
            if down != expected {
                bail!("Bad downstairs spot in Lot: {down}, closest match is {expected}");
            }
        }

        Ok(Lot {
            volume,
            sides,
            up,
            down,
        })
    }

    pub fn exit(&self, idx: usize) -> Option<Location> {
        let min = v3(self.volume.min());
        let mid_x = min.x + SECTOR_WIDTH / 4 * 2;
        let mid_y = min.y + SECTOR_HEIGHT / 4 * 2;

        match idx {
            0 => (self.sides & 0b1 != 0).then_some(min + ivec3(mid_x, 0, 0)),
            1 => (self.sides & 0b10 != 0)
                .then_some(min + ivec3(SECTOR_WIDTH - 1, mid_y, 0)),
            2 => (self.sides & 0b100 != 0)
                .then_some(min + ivec3(mid_x, SECTOR_HEIGHT - 1, 0)),
            3 => (self.sides & 0b1000 != 0).then_some(min + ivec3(0, mid_y, 0)),
            _ => panic!("Bad exit dir {idx}"),
        }
    }
}

#[derive(Clone, Debug, Default, Deref, DerefMut)]
pub struct Patch {
    #[deref]
    #[deref_mut]
    pub terrain: Cloud<3, Voxel>,
    pub spawns: IndexMap<Location, Spawn>,
}

impl Patch {
    pub fn from_sector_map(
        origin: &Location,
        value: &SectorMap,
    ) -> anyhow::Result<Self> {
        Ok(Patch {
            terrain: value.terrain(origin)?,
            spawns: value.spawns(origin)?.into_iter().collect(),
        })
    }
}

pub fn bigroom(rng: &mut dyn RngCore, lot: &Lot) -> anyhow::Result<Patch> {
    use Block::*;

    let mut ret = Patch::default();

    let floor = lot.volume.floor();

    for p in floor {
        let p = v3(p);
        ret.set_voxel(&p, None);
        ret.set_voxel(&p.below(), Some(Rock));
    }

    // TODO: Stairwells should have enclosures around them.
    if let Some(upstairs) = lot.up {
        ret.set_voxel(&upstairs.below(), Some(Rock));
    }

    if let Some(downstairs) = lot.down {
        ret.set_voxel(&downstairs, None);
        ret.set_voxel(&(downstairs + ivec3(0, 1, 0)), None);
    }

    let depth = 0.max(-lot.volume.min()[2]) as u32;
    let mobs = monster_spawns(depth);
    let items = item_spawns(depth);

    if !mobs.is_empty() {
        for _ in 0..10 {
            let pos: IVec3 = floor.sample(rng);
            let mob = mobs.choose_weighted(rng, |a| a.spawn_weight()).unwrap();
            ret.spawns.insert(pos, mob.clone());
        }
    }

    if !items.is_empty() {
        for _ in 0..10 {
            let pos: IVec3 = floor.sample(rng);
            let item =
                items.choose_weighted(rng, |a| a.spawn_weight()).unwrap();
            ret.spawns.insert(pos, item.clone());
        }
    }

    Ok(ret)
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Designation {
    /// Dig tunnel through cell to connect regions.
    Tunnel,
    /// Place door in cell to connect regions.
    Doorway,
    /// Do not dig cell
    Fixed,
    /// Cell is horizontal exit to other sector.
    ///
    /// Mostly works like `Fixed`, but a room's door can be placed here.
    Exit,
}

/// A versatile map generator function.
///
/// Parameters are in range [0.0, 1.0]. `roominess` describes how much of the
/// area should be covered by rooms (0: none other than entry/exit stairs to
/// 1: as many as possible). `loopiness` describes how many extra connections
/// are made beyond ones that connect all map parts (0: none to 1: every
/// possible one). `maziness` describes how much of the area between rooms is
/// filled with a maze with dead ends (0: only tunnels needed for connectivity
/// to 1: fill the entire area). `caviness` describes how much the tunnel
/// walls are eroded with a cellular automaton algorithm (0: none to 1: dig
/// out everything).
pub fn rooms_and_corridors(
    rng: &mut dyn RngCore,
    lot: &Lot,
    roominess: f32,
    loopiness: f32,
    maziness: f32,
    caviness: f32,
) -> anyhow::Result<Patch> {
    // XXX: This function is way too big, I should figure out some
    // intermediate abstractions for map generation and rewrite this in terms
    // of those. It's also a bit awkward to jump directly into the 3D voxel
    // cloud for map generation, this should probably be written in terms of
    // the simpler 2D tile map, though that'd mean giving up from being able
    // to do complex 3D features in this type of generator.

    use Designation::*;

    // How many times to run the smoothing cellular automaton for caves.
    const CAVE_CYCLES: usize = 2;

    assert!((0.0..=1.0).contains(&roominess));
    assert!((0.0..=1.0).contains(&loopiness));
    assert!((0.0..=1.0).contains(&maziness));
    assert!((0.0..=1.0).contains(&caviness));

    let floor = lot.volume.floor();
    let z = floor.min()[2];

    let mut ret = Patch::default();

    // Cells that can't be dug.
    let mut plan: HashMap<Location, Designation> = HashMap::default();

    let mut regions = HashMap::default();
    let mut region_idx = 0;

    // Set the horizontal exit cells.
    for dir in 0..4 {
        if let Some(exit) = lot.exit(dir) {
            ret.set_voxel(&exit, None);
            regions.insert(exit, region_idx);
            region_idx += 1;
            plan.insert(exit, Exit);
        }
    }

    // Return Option<usize> for floor area covered or None if placement wasn't
    // possible.
    let place_room = |r: &mut Patch,
                      plan: &mut HashMap<Location, Designation>,
                      regions: &mut HashMap<Location, usize>,
                      region_idx: &mut usize,
                      loc: Location,
                      border: &IndexMap<IVec2, char>,
                      inside: &IndexMap<IVec2, char>| {
        // Top corner must land at odd coords so that insides line up with
        // corridors.
        debug_assert!(loc.x.rem_euclid(2) == 1 && loc.y.rem_euclid(2) == 1);

        for &p in inside.keys() {
            let loc = loc + p.extend(0);

            if !floor.contains(loc) {
                return None;
            }

            if r.terrain.contains_key(&a3(loc)) {
                return None;
            }

            if plan.contains_key(&loc) {
                return None;
            }
        }

        // Check if there's showstoppers with border.
        for (p, &c) in border.iter() {
            let loc = loc + p.extend(0);

            if !floor.contains(loc) {
                return None;
            }

            if let Some(Exit) = plan.get(&loc) {
                if c == '#' {
                    // Trying to block sector exit with undiggable wall, no
                    // deal.
                    return None;
                }
            }
        }

        // Mutating actions commence here.

        for (p, &c) in border.iter() {
            let loc = loc + p.extend(0);

            // Only make edges if they can line up with a corridor along even
            // coordinates.
            let valid_edge = p.x.rem_euclid(2) == 0 || p.y.rem_euclid(2) == 0;

            let designation = match (plan.get(&loc), c) {
                (Some(Exit), _) => Exit,
                _ if !valid_edge => Fixed,
                (_, '#') => Fixed,
                (Some(Fixed), _) => Fixed,
                (Some(Doorway), _) => Doorway,
                (_, '+') => Doorway,
                _ => Tunnel,
            };

            // Open the exit right away.
            if designation == Exit {
                if c == '+' {
                    r.terrain.insert(loc, Some(Block::Door));
                } else {
                    r.terrain.insert(loc, None);
                }
            } else {
                // Otherwise just mark the plan with the wall designation.
                plan.insert(loc, designation);
            }
        }

        for (p, &c) in inside.iter() {
            let loc = loc + p.extend(0);

            loc.apply_char_terrain(&mut r.terrain, c)
                .expect("Bad SectorMap");
            plan.insert(loc, Fixed);
            regions.insert(loc, *region_idx);
        }

        *region_idx += 1;
        Some(inside.len() as i32)
    };

    // Place up and downstairs enclosures.
    if let Some(up) = lot.up {
        let loc = up + ivec3(-1, -1, -1);
        let room = SectorMap::upstairs();
        let (border, inside) = room.border_and_inside();
        place_room(
            &mut ret,
            &mut plan,
            &mut regions,
            &mut region_idx,
            loc,
            &border,
            &inside,
        )
        .expect("Failed to place stairwell");
    }

    if let Some(down) = lot.down {
        let loc = down + ivec3(-1, -1, 1);
        let room = SectorMap::downstairs();
        let (border, inside) = room.border_and_inside();
        place_room(
            &mut ret,
            &mut plan,
            &mut regions,
            &mut region_idx,
            loc,
            &border,
            &inside,
        )
        .expect("Failed to place stairwell");
    }

    // Generate rooms.
    let mut room_fill = (floor.volume() as f32 * roominess) as i32;
    let mut room_failure_budget = 10;
    'rooms: while room_fill > 0 && room_failure_budget > 0 {
        let room = rng.gen::<SectorMap>();

        // This part is expensive, so do it only once for every room.
        let (border, inside) = room.border_and_inside();

        // Try to fit it in.
        for _ in 0..32 {
            let mut loc: IVec3 = floor.sample(rng);
            // Snap to odd coords.
            loc.x = loc.x / 2 * 2 + 1;
            loc.y = loc.y / 2 * 2 + 1;
            if let Some(area) = place_room(
                &mut ret,
                &mut plan,
                &mut regions,
                &mut region_idx,
                loc,
                &border,
                &inside,
            ) {
                room_fill -= area;
                continue 'rooms;
            }
        }

        room_failure_budget -= 1;
    }

    for p in floor
        .into_iter()
        .filter(|[x, y, _]| x.rem_euclid(2) == 0 && y.rem_euclid(2) == 0)
        .filter(|&p| !plan.contains_key(&v3(p)))
    {
        let p = v3(p);

        ret.set_voxel(&p, None);
        regions.insert(p, region_idx);
        region_idx += 1;
    }

    // Find diggable 1-cell edges that connect two separate regions.
    let mut corridor_edges: Vec<(IVec3, [usize; 2])> = Vec::new();

    // Edges that have at least one room between them.
    let mut room_edges: Vec<(IVec3, [usize; 2])> = Vec::new();
    for p in floor {
        let p = v3(p);
        if regions.contains_key(&p) || plan.get(&p) == Some(&Fixed) {
            continue;
        }

        let mut regs = Vec::new();
        let mut is_room = false;
        for p in p.ns_4() {
            if let Some(i) = regions.get(&p) {
                if plan.get(&p) == Some(&Fixed) {
                    is_room = true;
                }

                regs.push(*i);
            }
        }

        if regs.len() == 2 && regs[0] != regs[1] {
            if is_room {
                room_edges.push((p, [regs[0], regs[1]]));
            } else {
                corridor_edges.push((p, [regs[0], regs[1]]));
            }
        }
    }

    room_edges.shuffle(rng);
    corridor_edges.shuffle(rng);

    // Put all corridor edges after all room edges, this way the corridor
    // system gets connected first, then corridors and rooms.
    let mut edges = room_edges
        .into_iter()
        .chain(corridor_edges)
        .collect::<Vec<_>>();

    let mut extra_edges = Vec::new();

    // Dig edges until map is connected.
    while let Some((p, [a, b])) = edges.pop() {
        // Open this one.
        if plan.get(&p) == Some(&Doorway) {
            ret.set_voxel(&p, Some(Block::Door));
        } else {
            ret.set_voxel(&p, None);
        }

        // Mark the region merge in the others.
        let mut rm_list = Vec::new();
        for (i, (p, [a2, b2])) in edges.iter_mut().enumerate().rev() {
            if *a2 == b {
                *a2 = a;
            }

            if *b2 == b {
                *b2 = a;
            }

            if a2 == b2 {
                extra_edges.push(*p);
                rm_list.push(i);
            }
        }

        for i in rm_list {
            edges.swap_remove(i);
        }
    }

    // Add some extra connections to sides
    if lot.sides & EAST != 0 {
        let min = v3(floor.min());
        for y in (0..SECTOR_HEIGHT).step_by(2) {
            let p1 = min + ivec3(SECTOR_WIDTH - 2, y, 0);
            let p2 = min + ivec3(SECTOR_WIDTH - 1, y, 0);

            if !plan.contains_key(&p1) && !plan.contains_key(&p2) {
                extra_edges.push(p2);
            }
        }
    }

    if lot.sides & SOUTH != 0 {
        let min = v3(floor.min());
        for x in (0..SECTOR_WIDTH).step_by(2) {
            let p1 = min + ivec3(x, SECTOR_HEIGHT - 2, 0);
            let p2 = min + ivec3(x, SECTOR_HEIGHT - 1, 0);

            if !plan.contains_key(&p1) && !plan.contains_key(&p2) {
                extra_edges.push(p2);
            }
        }
    }

    extra_edges.shuffle(rng);

    let n_loops = (extra_edges.len() as f32 * loopiness) as usize;
    for p in extra_edges.iter().take(n_loops) {
        if plan.get(p) == Some(&Doorway) {
            ret.set_voxel(p, Some(Block::Door));
        } else {
            ret.set_voxel(p, None);
        }
    }

    let dug = ret.terrain.len();
    let mut n_demaze = (dug as f32 * (1.0 - maziness)) as usize;

    'demaze: while n_demaze > 0 {
        let mut changed = false;
        let mut keys = ret.terrain.keys().copied().collect::<Vec<_>>();
        keys.shuffle(rng);
        for p in keys {
            let p = v3(p);
            if p.z != z || plan.contains_key(&p) {
                continue;
            }

            if p.ns_4()
                .filter(|p| ret.terrain.contains_key(&a3(*p)))
                .count()
                == 1
            {
                n_demaze -= 1;
                changed = true;
                // NB. It's important to manipulate the .terrain map directly
                // here so the removed tunnel goes back to "not touched by
                // mapgen" and is open to cave erosion later.
                ret.terrain.remove(p);
                if n_demaze == 0 {
                    break 'demaze;
                }
            }
        }
        if !changed {
            break;
        }
    }

    // Final stage: Dig some cave.

    // Get the set of cells you can dig for cave.
    let cave_area: IndexSet<Location> = floor
        .into_iter()
        .filter_map(|p| {
            if ret.terrain.contains_key(&p) || plan.contains_key(&v3(p)) {
                return None;
            }

            let p = v3(p);

            // Keep the sector sides intact unless there's a connection.
            if lot.sides & EAST == 0 && p.x == floor.max()[0] - 1 {
                return None;
            }
            if lot.sides & SOUTH == 0 && p.y == floor.max()[1] - 1 {
                return None;
            }

            Some(p)
        })
        .collect();

    let mut cave = IndexSet::default();

    // Randomly remove a bunch of cells.

    let mut holes = cave_area.iter().copied().collect::<Vec<_>>();
    holes.shuffle(rng);
    let dig_amount = (caviness * holes.len() as f32) as usize;
    for i in holes.into_iter().take(dig_amount) {
        cave.insert(i);
    }

    if caviness > 0.0 {
        // Smoothen cave with a 4-5 cellular automaton cycle.
        for _ in 0..CAVE_CYCLES {
            let mut cave_2 = cave.clone();
            for p in &cave_area {
                let filled_neighbors = p
                    .ns_8()
                    .filter(|p| {
                        cave.contains(p)
                            || ret.terrain.get(&a3(*p)) == Some(&None)
                    })
                    .count();

                // 4-5 rule
                if !cave.contains(p) && filled_neighbors >= 5 {
                    cave_2.insert(*p);
                }

                if cave.contains(p) && filled_neighbors < 4 {
                    cave_2.remove(p);
                }
            }
            cave = cave_2;
        }

        // Remove bubbles.
        if let Some(inside_point) = ret
            .terrain
            .keys()
            .find(|p| ret.terrain.get(*p) == Some(&None))
        {
            let valid_area = util::dijkstra_map(
                |p| {
                    p.ns_8().filter(|p| {
                        matches!(
                            ret.terrain.get(&a3(*p)),
                            Some(&None) | Some(&Some(Block::Door))
                        ) || cave.contains(&v3(*p))
                    })
                },
                vec![v3(*inside_point)],
            )
            .map(|(p, _)| p)
            .collect::<HashSet<Location>>();

            cave.retain(|p| valid_area.contains(p));
        }

        for p in cave {
            ret.terrain.set_voxel(&p, None);
        }
    }

    // Spawn creatures and items in open spots.
    let mut spawn_posns = ret
        .terrain
        .keys()
        .filter(|&[x, y, z]| {
            ret.terrain.get(&[*x, *y, *z]) == Some(&None)
                && ret.terrain.get(&[x + 1, *y, *z]) == Some(&None)
                && ret.terrain.get(&[*x, y + 1, *z]) == Some(&None)
                && ret.terrain.get(&[x + 1, y + 1, *z]) == Some(&None)
        })
        .copied()
        .map(v3)
        .flat_map(|p| {
            vec![
                p,
                p + ivec3(1, 0, 0),
                p + ivec3(0, 1, 0),
                p + ivec3(1, 1, 0),
            ]
        })
        // Deduplicate by collecting into IndexSet.
        .collect::<IndexSet<Location>>()
        .into_iter()
        .collect::<Vec<Location>>();
    spawn_posns.shuffle(rng);

    let depth = 0.max(-lot.volume.min()[2]) as u32;
    let mobs = monster_spawns(depth);
    let items = item_spawns(depth);

    if !mobs.is_empty() {
        for _ in 0..10 {
            let Some(pos) = spawn_posns.pop() else { break };
            let mob = mobs.choose_weighted(rng, |a| a.spawn_weight()).unwrap();
            ret.spawns.insert(pos, mob.clone());
        }
    }

    if !items.is_empty() {
        for _ in 0..10 {
            let Some(pos) = spawn_posns.pop() else { break };
            let item =
                items.choose_weighted(rng, |a| a.spawn_weight()).unwrap();
            ret.spawns.insert(pos, item.clone());
        }
    }

    Ok(ret)
}

fn monster_spawns(depth: u32) -> Vec<Spawn> {
    Data::get()
        .bestiary
        .iter()
        .filter(|(_, m)| m.min_depth() <= depth)
        .map(|(n, _)| n.parse().unwrap())
        .collect()
}

fn item_spawns(depth: u32) -> Vec<Spawn> {
    Data::get()
        .armory
        .iter()
        .filter(|(_, m)| m.min_depth() <= depth)
        .map(|(n, _)| n.parse().unwrap())
        .collect()
}
