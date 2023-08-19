use derive_deref::Deref;
use rand::{distributions::Standard, prelude::*};
use serde::{Deserialize, Serialize};

use crate::{prelude::*, Data, Patch};

/// Data that specifies the contents of an initial game world.
#[derive(Clone, Default, Deref, Deserialize, Serialize)]
pub struct World(IndexMap<Location, Patch>);

impl Distribution<World> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> World {
        // XXX: Placeholder thing, just spawn the first vault
        let patch = Data::get().vaults.iter().next().unwrap().1.clone();
        World([(Location::default(), patch)].into_iter().collect())
    }
}
