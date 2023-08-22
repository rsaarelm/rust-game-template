//! Special powers entities can use

use serde::{Deserialize, Serialize};

use crate::{ecs::Abilities, prelude::*};

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Ability {
    BerserkRage,
    CallLightning,
    Confusion,
    Firebolt,
}

use Ability::*;

impl Ability {
    pub fn needs_aim(self) -> bool {
        matches!(self, Firebolt)
    }

    pub fn invoke(
        self,
        _r: &mut Runtime,
        _loc: Location,
        _v: IVec2,
        _perp: Option<Entity>,
    ) {
        match self {
            BerserkRage => todo!(),
            CallLightning => todo!(),
            Confusion => todo!(),
            Firebolt => todo!(),
        }
    }
}

#[derive(
    Copy, Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize,
)]
#[serde(default, rename_all = "kebab-case")]
pub struct AbilityState {
    cooldown_until: Instant,
}

impl Entity {
    pub fn has_abilities(&self, r: &Runtime) -> bool {
        self.with::<Abilities, _>(r, |a| !a.0.is_empty())
    }

    pub fn abilities(&self, r: &Runtime) -> Vec<Ability> {
        self.with::<Abilities, _>(r, |ab| ab.0.keys().copied().collect())
    }

    pub(crate) fn cast(&self, r: &mut Runtime, ability: Ability, v: IVec2) {
        let Some(loc) = self.loc(r) else { return };
        ability.invoke(r, loc, v, Some(*self));
        self.complete_turn(r);
    }
}
