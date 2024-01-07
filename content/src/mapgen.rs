use anyhow::bail;
use derive_more::{Deref, DerefMut};
use glam::{ivec3, IVec3};
use rand::{distributions::Distribution, seq::SliceRandom, RngCore};
use util::{v3, Cloud, HashMap, IndexMap, Logos, Neighbors2D};

use crate::{
    data::GenericSector, world, Block, Coordinates, Cube, Data, Environs,
    Level, Location, SectorMap, Spawn, SpawnDist, Voxel, Zone,
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
    assert!((0.0..=1.0).contains(&roominess));
    assert!((0.0..=1.0).contains(&loopiness));
    assert!((0.0..=1.0).contains(&maziness));
    assert!((0.0..=1.0).contains(&caviness));

    let floor = lot.volume.floor();

    let mut ret = Patch::default();

    // TODO Support rooms, incl entry/exit

    let mut regions = HashMap::default();

    for (i, p) in floor
        .into_iter()
        .filter(|[x, y, _]| x.rem_euclid(2) == 0 && y.rem_euclid(2) == 0)
        .enumerate()
    {
        let p = v3(p);
        ret.set_voxel(&p, None);
        regions.insert(p, i);
    }

    // Find diggable 1-cell edges that connect two separate regions.
    let mut edges: Vec<(IVec3, [usize; 2])> = Vec::new();
    for p in floor {
        let p = v3(p);
        if regions.contains_key(&p) {
            continue;
        }

        let mut regs = Vec::new();
        for p in p.ns_4() {
            if let Some(i) = regions.get(&p) {
                regs.push(*i);
            }
        }

        if regs.len() == 2 && regs[0] != regs[1] {
            edges.push((p, [regs[0], regs[1]]));
        }
    }

    edges.shuffle(rng);

    let mut extra_edges = Vec::new();

    // Dig edges until map is connected.
    while let Some((p, [a, b])) = edges.pop() {
        // Open this one.
        ret.set_voxel(&p, None);

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

    let n_loops = (extra_edges.len() as f32 * loopiness) as usize;
    for p in extra_edges.iter().take(n_loops) {
        ret.set_voxel(p, None);
    }

    let dug = ret.terrain.len();
    let mut n_demaze = (dug as f32 * (1.0 - maziness)) as usize;

    'demaze: while n_demaze > 0 {
        let mut changed = false;
        for p in ret.terrain.keys().copied().collect::<Vec<_>>() {
            if v3(p)
                .ns_4()
                .filter(|p| ret.terrain.contains_key(&<[i32; 3]>::from(*p)))
                .count()
                == 1
            {
                n_demaze -= 1;
                changed = true;
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

    // TODO Dig some cellular automaton cave if caviness is requested

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
