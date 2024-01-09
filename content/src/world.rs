use std::collections::hash_map::Entry;

use anyhow::bail;
use glam::{ivec2, ivec3, IVec2};
use rand::prelude::*;
use serde::{Deserialize, Serialize, Serializer};
use static_assertions::const_assert;
use util::{a3, text, v2, v3, HashMap, Logos};

use crate::{
    data::Region, Block, Coordinates, Cube, Location, Lot, MapGenerator, Patch,
    Rect, Scenario, Spawn, Terrain, Voxel, Zone, LEVEL_DEPTH, SECTOR_HEIGHT,
    SECTOR_WIDTH,
};

/// Non-cached world data that goes in a save file.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct SerWorld {
    /// PRNG seed for the game.
    seed: Logos,
    /// Terrain that has been changed at runtime.
    overlay: Terrain,
    /// Sectors that have already had their entities spawned.
    spawn_history: Vec<Level>,
    /// Game scenario spec.
    scenario: Scenario,
}

#[derive(Default, Deserialize)]
#[serde(try_from = "SerWorld")]
pub struct World {
    /// Essential world information that is saved in savefiles.
    ///
    /// The rest of the content in `World` is computed cache values that can
    /// be derived from `inner`.
    inner: SerWorld,

    /// Procedurally generated main terrain store.
    ///
    /// This is the immutable terrain that's the direct result of procedural
    /// generation. Runtime alterations to terrain are stored in
    /// `inner.overlay`.
    terrain_cache: Terrain,

    // NB. Skeleton looks like you could just put a
    //
    // #[serde(try_from = "Scenario")]
    //
    // on it, but there's a wrinkle that it's going to need randomness from
    // the world seed as well during construction. Most of the randomness is
    // punted to to the actual level generators skeleton only references, but
    // there are dungeon structure features where a dungeon branch's length
    // can randomly vary, which are decided during skeleton construction.
    //
    // This means that skeleton construction needs access to the whole
    // SerWorld type.
    /// Built from scenario.
    skeleton: HashMap<Level, Segment>,

    gen_status: HashMap<Level, GenStatus>,

    player_entrance: Location,
}

// Do this manually because otherwise I get complaints about no Clone impl
// even though I'm serializing via substruct.
impl Serialize for World {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.inner.serialize(serializer)
    }
}

enum GenStatus {
    /// Generated, but it's surroudnings haven't been.
    Edge,
    /// Generated, and surroundings have been generated, ignore.
    Core,
}

// All connections must have a segment on both sides, so any segment needs to
// only specify half of the potential connections, the other halves are found
// on the opposing segment.

pub struct Segment {
    pub connected_north: bool,
    pub connected_west: bool,
    pub connected_down: Option<Location>,
    pub generator: Box<dyn MapGenerator>,
}

impl From<World> for SerWorld {
    fn from(value: World) -> Self {
        value.inner
    }
}

impl TryFrom<SerWorld> for World {
    type Error = anyhow::Error;

    fn try_from(value: SerWorld) -> Result<Self, Self::Error> {
        // TODO: Build skeleton
        // Can build the whole terrain here while we keep things simple and
        // don't do gradual runtime worldbuild...

        let (player_entrance, skeleton) =
            build_skeleton(&value.seed, &value.scenario)?;

        Ok(World {
            inner: value,
            skeleton,
            player_entrance,
            ..Default::default()
        })
    }
}

fn build_skeleton(
    seed: &Logos,
    scenario: &Scenario,
) -> anyhow::Result<(Location, HashMap<Level, Segment>)> {
    use Region::*;

    let mut start_pos = None;
    let mut skeleton = HashMap::default();

    for (p, c) in text::char_grid(&scenario.map) {
        let Some(stack) = scenario.legend.get(&c) else {
            bail!("Unknown overworld char {c:?}");
        };
        let z = -1
            + stack.iter().take_while(|a| a.is_above_ground()).count() as i32;

        for (depth, region) in stack.iter().enumerate() {
            let s = Level::level_at([p.x, p.y, z - (depth as i32)]);
            let origin = Location::from(s.min());

            let at_bottom = depth == stack.len() - 1;

            let segment = match region {
                Generate(gen) => Segment {
                    connected_north: false,
                    connected_west: false,
                    connected_down: if !at_bottom {
                        Some(default_down_stairs(seed, s))
                    } else {
                        None
                    },
                    generator: Box::new(*gen),
                },
                Site(map) | Hall(map) => {
                    for p in map.entrances() {
                        if start_pos.is_none() {
                            start_pos = Some(origin + p.extend(0));
                        } else {
                            bail!("Scenario defines more than one start location.");
                        }
                    }

                    Segment {
                        connected_north: false,
                        connected_west: false,
                        connected_down: map
                            .find_downstairs()
                            .map(|p| origin + p.extend(-1)),
                        generator: Box::new(Patch::from_sector_map(
                            &origin, map,
                        )?),
                    }
                }
                Branch(_stack) => todo!(),
                Repeat(_n, _gen) => todo!(),
            };

            skeleton.insert(s, segment);
        }
    }

    let Some(start_pos) = start_pos else {
        bail!("No player start pos specified.");
    };

    Ok((start_pos, skeleton))
}

impl World {
    pub fn new(seed: Logos, scenario: Scenario) -> anyhow::Result<Self> {
        let (player_entrance, skeleton) = build_skeleton(&seed, &scenario)?;

        Ok(World {
            inner: SerWorld {
                seed,
                scenario,
                ..Default::default()
            },
            skeleton,
            player_entrance,
            ..Default::default()
        })
    }

    pub fn seed(&self) -> &Logos {
        &self.inner.seed
    }

    pub fn populate_around(
        &mut self,
        loc: &Location,
    ) -> Vec<(Location, Spawn)> {
        let s = Level::level_from(loc);

        // Early exit if this is already a core generated sector.
        if matches!(self.gen_status.get(&s), Some(&GenStatus::Core)) {
            return Default::default();
        }

        let mut spawns = Vec::new();

        for s in s.cache_volume() {
            self.generate_sector(&s, &mut spawns);
        }

        // Mark the center sector as core, exit early when populate is called
        // again on it.
        self.gen_status.insert(s, GenStatus::Core);

        spawns
    }

    fn generate_sector(
        &mut self,
        s: &Level,
        spawns: &mut Vec<(Location, Spawn)>,
    ) {
        match self.gen_status.entry(*s) {
            // This sector has already been generated, do nothing.
            Entry::Occupied(_) => return,
            // Mark sector as generated.
            Entry::Vacant(e) => e.insert(GenStatus::Edge),
        };

        let spawns_done = self.inner.spawn_history.contains(s);

        let Some(segment) = self.skeleton.get(s) else {
            // This sector does not belong in the defined game world, bail
            // out.
            return;
        };

        let lot = self.construct_lot(s);
        self.inner.spawn_history.push(*s);

        log::info!(
            "Generating {s:?}{}",
            if spawns_done {
                " (skipping spawns)"
            } else {
                ""
            }
        );

        let mut rng = util::srng(&(&self.inner.seed, s));
        let patch = segment
            .generator
            .run(&mut rng, &lot)
            .expect("Sector procgen failed");

        for (loc, block) in patch.terrain.iter() {
            if *block != self.default_terrain(&v3(*loc)) {
                self.terrain_cache.insert(*loc, *block);
            }
        }

        if !spawns_done {
            spawns.extend(patch.spawns);
        }
    }

    fn construct_lot(&self, s: &Level) -> Lot {
        let mut sides =
            self.skeleton.get(s).map_or(0, |a| a.connected_north as u8);
        sides |= self
            .skeleton
            .get(&s.east())
            .map_or(0, |a| a.connected_west as u8)
            << 1;
        sides |= self
            .skeleton
            .get(&s.south())
            .map_or(0, |a| a.connected_north as u8)
            << 2;
        sides |=
            self.skeleton.get(s).map_or(0, |a| a.connected_west as u8) << 3;

        let up = self.skeleton.get(&s.above()).and_then(|a| a.connected_down);
        let down = self.skeleton.get(s).and_then(|a| a.connected_down);

        let volume = *s;

        Lot::new(volume, sides, up, down).unwrap()
    }

    pub fn player_entrance(&self) -> Location {
        self.player_entrance
    }

    pub fn get(&self, loc: &Location) -> Voxel {
        let pt = a3(*loc);
        if let Some(&mutated) = self.inner.overlay.get(&pt) {
            return mutated;
        }

        if let Some(&cached) = self.terrain_cache.get(&pt) {
            return cached;
        }

        self.default_terrain(loc)
    }

    pub fn set(&mut self, loc: &Location, voxel: Voxel) {
        self.inner.overlay.insert(*loc, voxel);
    }

    fn default_terrain(&self, loc: &Location) -> Voxel {
        if loc.z >= 0 {
            None
        } else {
            Some(Block::Rock)
        }
    }
}

// NB. This is specifically the sort of Cube you get from Zone::level(), but
// there's currently no type wrapping to enforce it. Let's see if things can
// work like this without a mess-up.
pub type Level = Cube;

/// Fixed downstairs positions for every level given a world seed.
///
/// The up and down stairwell positions generated using this method are
/// guaranteed to be apart for every level.
pub fn default_down_stairs(seed: &Logos, s: Level) -> Location {
    snap_stairwell_position(
        (Cube::from(s).border([0, 0, -1]) + ivec3(0, 0, -1))
            .sample(&mut util::srng(&(seed, s))),
    )
}

/// Snaps a stairwell position to its closest designated grid position for its
/// Z-level.
///
/// To keep up and down stairs for a random level from ending up on the same
/// x,y and creating an ungenerateable map, stairwells on consecutive levels
/// must alternate between black and white "chessboard squares" of a grid of
/// 3x3 cells. Stairwells are also kept away from the very edge of the sector.
pub(crate) fn snap_stairwell_position(loc: Location) -> Location {
    // Find dimensions for the chessboard zone, leave some space at edges.
    const W: i32 = (SECTOR_WIDTH - 2) / 8 * 8;
    const H: i32 = (SECTOR_HEIGHT - 2) / 8 * 8;

    // Sector dimensions too small for stairwell placement if this trips.
    const_assert!(W > 0 && H > 0);

    // Offset of the chessboard zone off sector edge. Make sure the offset
    // coordinates are even, stairwells should snap to even positions.
    const X: i32 = (SECTOR_WIDTH - W) / 4 * 2;
    const Y: i32 = (SECTOR_HEIGHT - H) / 4 * 2;

    // Place the chessboard zone in location's sector.
    let bounds =
        Rect::new([X, Y], [X + W, Y + H]) + loc.sector_snap_2d().truncate();

    // Use location sector for parity, alternate valid squares for every other
    // sector, and snap to the position.
    snap_to_chessboard3(loc.z.div_floor(LEVEL_DEPTH), &bounds, loc.truncate())
        .extend(loc.z)
}

/// Snap a point to the center of 4x4 "chessboard" squares within the area of
/// `bounds`. (Why 4x4 instead of 3x3? Because we might have convenience
/// conventions in generators that place corridors in even coordinates, and
/// stairwells should fit in the pattern)
///
/// The point is snapped to "white" or "black" squares based on whether
/// `parity` is even or odd.
fn snap_to_chessboard3(parity: i32, bounds: &Rect, pos: IVec2) -> IVec2 {
    // Chessboard square size.
    const N: i32 = 4;
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
