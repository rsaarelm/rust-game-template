use rand::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    ecs::{
        Abilities, Icon, IsFriendly, IsMob, Level, Name, Nickname, Speed,
        Stats, Voice,
    },
    prelude::*,
    Goal,
};

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
        c: &mut Core,
        loc: Location,
        v: IVec2,
        perp: Option<Entity>,
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
    pub fn has_abilities(&self, c: &Core) -> bool {
        self.with::<Abilities, _>(c, |a| !a.0.is_empty())
    }

    pub fn abilities(&self, c: &Core) -> Vec<Ability> {
        self.with::<Abilities, _>(c, |ab| ab.0.keys().copied().collect())
    }

    pub(crate) fn cast(&self, c: &mut Core, ability: Ability, v: IVec2) {
        let Some(loc) = self.loc(c) else { return };
        ability.invoke(c, loc, v, Some(*self));
        self.complete_turn(c);
    }
}
