use std::collections::BTreeMap;

use derive_more::{Add, Deref, From};
use glam::{ivec3, IVec3};
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use util::{v2, Logos};

use crate::{mapgen::Level, prelude::*, Cube, OldPatch, Patch, Rect, Spawn};

#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Hash,
    Debug,
    Deref,
    Add,
    From,
    Serialize,
    Deserialize,
)]
pub struct Sector(IVec3);

impl From<Location> for Sector {
    fn from(value: Location) -> Self {
        Sector(ivec3(
            (value.x() as i32).div_floor(SECTOR_WIDTH),
            (value.y() as i32).div_floor(SECTOR_HEIGHT),
            (value.z() as i32).div_floor(SECTOR_DEPTH),
        ))
    }
}

impl From<Sector> for Cube {
    fn from(value: Sector) -> Self {
        let sector_size = ivec3(SECTOR_WIDTH, SECTOR_HEIGHT, SECTOR_DEPTH);
        let origin = *value * ivec3(SECTOR_WIDTH, SECTOR_HEIGHT, SECTOR_DEPTH);
        Cube::new(origin, origin + sector_size)
    }
}

/// Fixed-format data that specifies the contents of the initial game world.
/// Created from `WorldSpec`.
#[derive(Clone, Default)]
#[deprecated]
pub struct OldWorld {
    /// PRNG seed used
    seed: Logos,
    /// Map generation artifacts specifying terrain and entity spawns.
    patches: IndexMap<Location, OldPatch>,
    /// Replicates data from `patches` in a more efficiently accessible form.
    terrain: HashMap<Location, MapTile>,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct World {
    /// PRNG seed used.
    seed: Logos,
    /// Scenario data used to initialize this world.
    scenario: Region,
    /// Entities that have been spawned once from this world.
    spawn_history: IndexSet<(Location, Spawn)>,
    /// Terrain that has been changed during runtime.
    // TODO: Use an atlas type here to save it neatly
    terrain_overlay: HashMap<Location, Voxel>,

    #[serde(skip)]
    skeleton: Skeleton,
    #[serde(skip)]
    terrain_cache: HashMap<Location, Voxel>,
}

/// Snaps a stairwell position to its closest designated grid position for its
/// Z-level.
///
/// To keep up and down stairs for a random level from ending up on the same
/// x,y and creating an ungenerateable map, stairwells must alternate between
/// black and white chessboard squares of a grid of 3x3 cells. Stairwells are
/// also kept away from the very edge of the sector.
fn snap_stairwell_position(pos: IVec3) -> IVec3 {
    // TODO: Find largest chessboard box that's multiples of 6 large and fits
    // inside a sector with at least one tile layer at the boundaries
    let bounds = todo!();

    snap_to_chessboard3(pos.z.div_floor(2), &bounds, pos.truncate())
        .extend(pos.z)
}

/// Snap a point to the center of 3x3 "chessboard" squares within the area of
/// `bounds`.
///
/// The point is snapped to "white" or "black" squares based on whether
/// `parity` is even or odd.
fn snap_to_chessboard3(parity: i32, bounds: &Rect, pos: IVec2) -> IVec2 {
    // Chessboard square size.
    const N: i32 = 3;
    const N2: i32 = N * 2;

    assert!(
        {
            let [w, h] = bounds.dim();
            w > 0 && w % N2 == 0 && h > 0 && h % N2 == 0
        },
        "snap_to_chessboard3: bounds dimensions must be nonzero multiples of {N2}"
    );

    // Figure out the chessboard color the point falls.
    let origin = v2(bounds.min());
    let tile = pos - origin;
    let color = (tile.x.div_floor(N) + tile.y.div_floor(N)).rem_euclid(2);

    // Displace it to the next square over if it falls on the wrong pos for
    // the current parity.
    let adjusted_pos = if color != parity.rem_euclid(2) {
        pos + ivec2(N, 0)
    } else {
        pos
    };

    // Snap point to center of square.
    let tile = adjusted_pos - origin;
    let adjusted_pos = origin
        + ivec2(
            tile.x.div_floor(N) * N + N / 2,
            tile.y.div_floor(N) * N + N / 2,
        );

    // Finally wrap it to the bounds of the chessboard and we're done.
    bounds.mod_proj(adjusted_pos)
}

impl World {
    pub fn new(seed: Logos, scenario: Region) -> anyhow::Result<Self> {
        // Fail construction if new World is created with an invalid scenario.
        let mut ret = World {
            seed,
            scenario,
            ..Default::default()
        };
        ret.construct_skeleton()?;
        Ok(ret)
    }

    /// Populate the world cache around the given location.
    ///
    /// The world cache will not have contents until the populate method is
    /// called. The method will also return a list of entities that need to be
    /// spawned in the area surrounding the location.
    ///
    /// Calling this repeatedly for the same location will exit quickly and
    /// will not cause further entity spawn requests to fire. You are expected
    /// to call this around the current player position every frame.
    pub fn populate_around(&mut self, loc: Location) -> Vec<(Location, Spawn)> {
        // We can get a World via deserialization that has an undetected
        // invalid scenario, it will cause a panic at this point.
        self.construct_skeleton().expect("Invalid scenario data");
        todo!()
    }

    fn construct_skeleton(&mut self) -> anyhow::Result<()> {
        if self.skeleton.is_empty() {
            self.skeleton =
                Skeleton::new(&mut util::srng(&self.seed), &self.scenario)?;
        }
        Ok(())
    }

    pub fn voxel(&self, loc: Location) -> Voxel {
        if let Some(&mutated) = self.terrain_overlay.get(&loc) {
            return mutated;
        }

        if let Some(&cached) = self.terrain_cache.get(&loc) {
            return cached;
        }

        // Default terrain, solid rock underground and empty air overground.
        if loc.z() < 0 {
            Some(Block::Rock)
        } else {
            None
        }
    }
}

impl TryFrom<WorldSpec> for OldWorld {
    type Error = anyhow::Error;

    fn try_from(value: WorldSpec) -> Result<Self, Self::Error> {
        let mut rng = util::srng(&value.seed);

        let mut patches: IndexMap<Location, OldPatch> = IndexMap::default();

        /*
        const MAX_DEPTH: u32 = 8;

        let mut prev_downstairs = None;
        for depth in 1..=MAX_DEPTH {
            let mut level = Level::new(depth);
            if depth < MAX_DEPTH {
                level = level.with_downstairs();
            }
            if let Some(p) = prev_downstairs {
                level = level.upstairs_at(p + ivec2(0, -1));
            }

            let map = level.sample(&mut rng);
            prev_downstairs = map.downstairs_pos();

            let z = -(depth as i16);
            patches.insert(Location::new(0, 0, z), map);
        }
        */

        let mut terrain = HashMap::default();
        for (&loc, a) in &patches {
            for (pos, t) in a.tiles() {
                terrain.insert(loc + pos, t);
            }
        }

        Ok(OldWorld {
            seed: value.seed,
            patches,
            terrain,
        })
    }
}

impl OldWorld {
    pub fn spawns(&self) -> impl Iterator<Item = (Location, &'_ Spawn)> + '_ {
        self.patches.iter().flat_map(|(&loc, a)| {
            a.spawns.iter().map(move |(&p, s)| (loc + p, s))
        })
    }

    pub fn tile(&self, loc: &Location) -> Option<MapTile> {
        self.terrain.get(loc).copied()
    }

    pub fn seed(&self) -> &Logos {
        &self.seed
    }

    pub fn entrance(&self) -> Option<Location> {
        for (&loc, a) in &self.patches {
            if let Some(pos) = a.entrance {
                return Some(loc + pos);
            }
        }
        None
    }
}

/// Compact description of what the initial game world is like. Will be stored
/// in save files. Expansion is highly context-dependent, may use prefab maps
/// or procedural generation.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[deprecated]
pub struct WorldSpec {
    seed: Logos,
}

impl WorldSpec {
    pub fn new(seed: Logos) -> Self {
        WorldSpec { seed }
    }
}

#[derive(Copy, Clone, Debug)]
enum GenerationStatus {
    /// Sector and its surroundings have been generated. When a `Core` sector is
    /// queried, nothing is done.
    Core,
    /// Sector is generated, but is at the edge of ungenerated space. When a
    /// `Rim` sector is queried, it is made into `Core` and further `Rim`
    /// sectors are generated around it.
    Rim,
}

/// Internal representation of the region spec, a world skeleton.
#[derive(Default)]
struct Skeleton {
    /// Sectors that have been generated.
    sector_status: HashMap<Sector, GenerationStatus>,

    generators: HashMap<Sector, Box<dyn MapGenerator>>,
}

impl Skeleton {
    pub fn new(
        rng: &mut (impl Rng + ?Sized),
        scenario: &Region,
    ) -> anyhow::Result<Self> {
        todo!()
    }

    /// Call this to quickly determine if the skeleton hasn't been initialized
    /// yet after loading a game.
    pub fn is_empty(&self) -> bool {
        todo!()
    }
}

pub trait MapGenerator {
    fn run(&self, rng: &mut dyn RngCore, lot: &Lot) -> anyhow::Result<Patch>;
}

/// Bounds and topology definition for map generation.
pub struct Lot {
    /// Volume in space in which the map should be generated.
    volume: Cube,

    /// Connection flags to the six neighbors. If the bit is set for a given
    /// edge, the map generator is expected to generate a connection in that
    /// direction. The bit order is NESWDU.
    connections: u8,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case", from = "((SerRegion,), String)")]
pub struct Region {
    map: String,
    legend: BTreeMap<char, Vec<RegionSegment>>,
}

impl From<((SerRegion,), String)> for Region {
    fn from(((value,), map): ((SerRegion,), String)) -> Self {
        let mut ret = Region::from(value);

        if !map.trim().is_empty() && ret.map.trim().is_empty() {
            ret.map = map;
        }

        ret
    }
}

impl From<SerRegion> for Region {
    fn from(value: SerRegion) -> Self {
        Region {
            map: value.map,
            legend: value.legend,
        }
    }
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct SerRegion {
    map: String,
    legend: BTreeMap<char, Vec<RegionSegment>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegionSegment {
    /// A procgen level
    Generate(MapGen),
    /// An above-ground prefab level
    Site(((PatchData,), String)),
    /// An underground prefab level
    Vault(((PatchData,), String)),
    /// Branch a new stack off to the side
    Branch(Vec<RegionSegment>),
    /// A sequence of applying the same constructor multiple times.
    Repeat(u32, Box<RegionSegment>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MapGen {
    Water,
    Grassland,
    Forest,
    Mountains,
    Dungeon,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct PatchData {
    name: String,
    legend: BTreeMap<char, Spawn>,
}
