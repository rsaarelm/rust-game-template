use content::{Item, Monster};

use crate::{ecs::*, prelude::*};

/// Static entity descriptors that specify runtime entities.
pub trait EntitySpec {
    fn build(&self, r: &mut Runtime, name: &str) -> Entity;
}

impl EntitySpec for Monster {
    fn build(&self, r: &mut Runtime, name: &str) -> Entity {
        Entity(r.ecs.spawn((
            Name(name.into()),
            Icon(self.icon),
            Speed(3),
            Level(self.might),
            IsMob(true),
            Stats {
                hit: self.might,
                ev: self.might / 2,
                dmg: self.might,
            },
        )))
    }
}

impl EntitySpec for Item {
    fn build(&self, r: &mut Runtime, name: &str) -> Entity {
        Entity(r.ecs.spawn((
            Name(name.into()),
            Icon(self.kind.icon()),
            ItemPower(self.power.clone()),
            self.kind,
            Level(self.might),
        )))
    }
}
