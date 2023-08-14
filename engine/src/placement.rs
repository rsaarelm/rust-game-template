use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::prelude::*;

#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Debug,
    Serialize,
    Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Place {
    In(Entity),
    At(Location),
}

use Place::*;

impl From<Entity> for Place {
    fn from(e: Entity) -> Self {
        In(e)
    }
}

impl From<Location> for Place {
    fn from(loc: Location) -> Self {
        At(loc)
    }
}

impl Into<Option<Entity>> for Place {
    fn into(self) -> Option<Entity> {
        if let In(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

impl Into<Option<Location>> for Place {
    fn into(self) -> Option<Location> {
        if let At(loc) = self {
            Some(loc)
        } else {
            None
        }
    }
}

/// Spatial index, used for efficiently finding locations of entities and
/// entities at locations.
#[derive(Clone, Default, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(from = "BTreeMap<Entity, Place>", into = "BTreeMap<Entity, Place>")]
pub struct Placement {
    places: BTreeMap<Entity, Place>,
    entities: IndexMap<Place, IndexSet<Entity>>,
}

impl Placement {
    pub fn entities_at(
        &self,
        loc: Location,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.entities
            .get(&Place::from(loc))
            .into_iter()
            .flatten()
            .copied()
    }

    pub fn entities_in(
        &self,
        container: &Entity,
    ) -> impl Iterator<Item = Entity> + '_ {
        self.entities
            .get(&Place::from(*container))
            .into_iter()
            .flatten()
            .copied()
    }

    pub fn all_entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.places.keys().cloned()
    }

    pub fn entity_pos(&self, e: &Entity) -> Option<Location> {
        match self.places.get(e) {
            None => None,
            Some(At(loc)) => Some(*loc),
            Some(In(e)) => self.entity_pos(e),
        }
    }

    pub fn remove(&mut self, e: &Entity) {
        if let Some(loc) = self.places.get(e).copied() {
            self.places.remove(e);
            if let Some(set) = self.entities.get_mut(&loc) {
                set.shift_remove(e);
            }
            // XXX: Should we remove the `entities_at` bins as they get
            // emptied? Is neater memory management, but in practice the same
            // bins will get emptied and filled a lot, so it probably reduces
            // churn just to leave them in place.
        }
    }

    pub fn contains(&self, container: &Entity, e: &Entity) -> bool {
        for i in self.entities_in(container) {
            if e == &i {
                return true;
            }
            if self.contains(&i, e) {
                return true;
            }
        }
        false
    }

    pub fn insert_at(&mut self, loc: Location, e: Entity) {
        self.insert(loc.into(), e);
    }

    pub fn insert_in(&mut self, container: &Entity, e: Entity) {
        assert!(
            *container != e && !self.contains(&e, container),
            "Placement::insert_in: Containment loop"
        );
        self.insert((*container).into(), e);
    }

    fn insert(&mut self, place: Place, e: Entity) {
        self.remove(&e);
        self.places.insert(e, place);
        self.entities.entry(place).or_default().insert(e);
    }
}

impl From<BTreeMap<Entity, Place>> for Placement {
    fn from(s: BTreeMap<Entity, Place>) -> Self {
        let mut ret = Self::default();
        for (e, p) in s {
            ret.insert(p, e);
        }
        ret
    }
}

impl Into<BTreeMap<Entity, Place>> for Placement {
    fn into(self) -> BTreeMap<Entity, Place> {
        self.places
    }
}
