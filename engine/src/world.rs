use std::collections::BTreeMap;

use rand::{distributions::Standard, prelude::*};
use serde::Deserialize;

use crate::{prelude::*, Atlas, Prototypes};

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct World {
    pub terrain: Atlas,
    pub legend: IndexMap<char, String>,
    pub lexicon: Prototypes,
}

impl Distribution<World> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> World {
        let mut terrain: BTreeMap<Location, char> = BTreeMap::new();

        for p in Location::default().sector_bounds() {
            let loc = Location::from(IVec2::from(p));
            if loc.at_sector_edge() || rng.gen_range(0..5) == 0 {
                terrain.insert(loc, '#');
            } else {
                terrain.insert(loc, '.');
            }
        }

        terrain.insert(Location::default().sector() + ivec2(10, 10), '@');

        World {
            terrain: Atlas::from_iter(terrain),
            ..Default::default()
        }
    }
}
