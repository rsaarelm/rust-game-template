use anyhow::bail;
use derive_more::{Deref, DerefMut};
use glam::{ivec3, IVec3};
use rand::{distributions::Distribution, seq::SliceRandom, RngCore};
use util::{v3, Cloud, IndexMap, Logos};

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

    let floor = lot.volume.border([0, 0, -1]);

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
