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
        // TODO: Generate multiple floors when up and downstairs gen is
        // working.
        ret.insert(Location::new(0, 0, -1), Level::new(1).sample(rng));
        World(ret)
    }
}
