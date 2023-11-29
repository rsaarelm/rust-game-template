use derive_more::{Deref, DerefMut};
use glam::{ivec3, IVec3};
use rand::{prelude::*, seq::SliceRandom, RngCore};
use util::{Cloud, IndexMap};

use crate::{
    data::GenericSector, Cube, Data, Environs, Location, SectorMap, Spawn,
    SpawnDist, Tile2D,
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
#[derive(Copy, Clone, Debug, Default)]
pub struct Lot {
    /// Volume in space in which the map should be generated.
    pub volume: Cube,

    /// Connection flags to the four horizontal neighbors. The bit order is
    /// NESW.
    pub sides: u8,

    pub up: Option<Location>,
    pub down: Option<Location>,
}

#[derive(Clone, Debug, Default, Deref, DerefMut)]
pub struct Patch {
    #[deref]
    #[deref_mut]
    pub terrain: Cloud<3, Tile2D>,
    pub spawns: IndexMap<Location, Spawn>,
}

impl Patch {
    pub fn from_sector_map(
        origin: Location,
        value: &SectorMap,
    ) -> anyhow::Result<Self> {
        Ok(Patch {
            terrain: value.terrain(origin)?,
            spawns: value.spawns(origin)?.into_iter().collect(),
        })
    }
}

pub fn bigroom(rng: &mut dyn RngCore, lot: &Lot) -> anyhow::Result<Patch> {
    let mut ret = Patch::default();

    let floor = lot.volume.border([0, 0, -1]);

    for p in floor {
        ret.set_tile(p.into(), Tile2D::Ground);
    }

    let z = lot.volume.max()[2] - 1;

    // TODO: These should have wall enclosures around them.

    if let Some(mut upstairs) = lot.up {
        upstairs.z = z;
        ret.set_tile(upstairs + ivec3(0, -1, 0), Tile2D::Upstairs);
    }

    if let Some(mut downstairs) = lot.down {
        downstairs.z = z;
        ret.set_tile(downstairs, Tile2D::Downstairs);
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
