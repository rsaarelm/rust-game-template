use derive_deref::Deref;
use rand::{distributions::Standard, prelude::*};
use serde::{Deserialize, Serialize};

use crate::{mapgen::Level, prelude::*, Patch};

/// Data that specifies the contents of an initial game world.
#[derive(Clone, Default, Deref, Deserialize, Serialize)]
pub struct World(IndexMap<Location, Patch>);

impl Distribution<World> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> World {
        let mut ret = IndexMap::default();
        let mut prev_downstairs = None;
        for depth in 1..=20 {
            // TODO: Up & downstairs
            let mut level = Level::new(depth);
            if depth != 20 {
                level = level.with_downstairs();
            }
            if let Some(p) = prev_downstairs {
                level = level.upstairs_at(p);
            }
            let patch = level.sample(rng);
            if let Some(downstairs) = patch.downstairs_pos() {
                prev_downstairs = Some(downstairs);
            }
            ret.insert(Location::new(0, 0, -(depth as i16)), patch);
        }
        World(ret)
    }
}
