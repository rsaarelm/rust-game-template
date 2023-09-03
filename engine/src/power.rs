//! Special powers entities can use

use serde::{Deserialize, Serialize};

use crate::{ecs::Powers, prelude::*};

#[derive(
    Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Power {
    BerserkRage,
    CallLightning,
    Confusion,
    Fireball,
    MagicMapping,
}

use Power::*;

impl Power {
    pub fn needs_aim(self) -> bool {
        matches!(self, Fireball)
    }

    pub fn invoke(
        self,
        _r: &mut Runtime,
        _loc: Location,
        _v: IVec2,
        _perp: Option<Entity>,
    ) {
        match self {
            BerserkRage => msg!("TODO!"),
            CallLightning => msg!("TODO!"),
            Confusion => msg!("TODO!"),
            Fireball => msg!("TODO!"),
            MagicMapping => msg!("TODO!"),
        }
    }
}

#[derive(
    Copy, Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize,
)]
#[serde(default, rename_all = "kebab-case")]
pub struct PowerState {
    cooldown_until: Instant,
}

impl Entity {
    pub fn has_powers(&self, r: &Runtime) -> bool {
        self.with::<Powers, _>(r, |a| !a.0.is_empty())
    }

    pub fn powers(&self, r: &Runtime) -> Vec<Power> {
        self.with::<Powers, _>(r, |ab| ab.0.keys().copied().collect())
    }

    pub(crate) fn cast(&self, r: &mut Runtime, power: Power, v: IVec2) {
        let Some(loc) = self.loc(r) else { return };
        power.invoke(r, loc, v, Some(*self));
        self.complete_turn(r);
    }
}
