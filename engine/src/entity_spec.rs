use world::{Item, Monster};

use crate::{ecs::*, prelude::*};

/// Static entity descriptors that specify runtime entities.
pub trait EntitySpec {
    fn build(&self, r: &mut Runtime, name: &str) -> Entity;
}

impl EntitySpec for Monster {
    fn build(&self, r: &mut Runtime, name: &str) -> Entity {
        let ret = Entity(r.ecs.spawn((
            Name(name.into()),
            Icon(self.icon),
            Speed(3),
            IsMob(true),
            Stats {
                level: self.level,
                hit: 0,
                ev: self.evasion,
                dmg: self.attack_damage,
            },
        )));

        // If we indiscriminately set flags in the build expression, empty
        // flag sets will show up as values in ECS.
        if !self.flags.is_empty() {
            ret.set(r, self.flags);
        }

        ret
    }
}

impl EntitySpec for Item {
    fn build(&self, r: &mut Runtime, name: &str) -> Entity {
        let ret = Entity(r.ecs.spawn((
            Name(name.into()),
            Icon(self.kind.icon()),
            ItemPower(self.power.clone()),
            self.kind,
            Stats {
                level: self.level,
                ..Default::default()
            },
        )));
        if self.kind.is_stacking() {
            ret.set(r, Count(1));
        }
        ret
    }
}
